// Copyright Valkey GLIDE Project Contributors - SPDX Identifier: Apache-2.0

use glide_core::errors::error_message;
use glide_core::start_socket_listener as start_socket_listener_core;
// Protocol constants to expose to Java.
use glide_core::HASH as TYPE_HASH;
use glide_core::LIST as TYPE_LIST;
use glide_core::MAX_REQUEST_ARGS_LENGTH as MAX_REQUEST_ARGS_LENGTH_IN_BYTES;
use glide_core::SET as TYPE_SET;
use glide_core::STREAM as TYPE_STREAM;
use glide_core::STRING as TYPE_STRING;
use glide_core::ZSET as TYPE_ZSET;
use glide_core::client::FINISHED_SCAN_CURSOR;

// Telemetry required for getStatistics
use glide_core::Telemetry;

use bytes::Bytes;
use jni::JNIEnv;
use jni::errors::Error as JniError;
use jni::objects::{JByteArray, JClass, JObject, JObjectArray, JString};
use jni::sys::{jint, jlong, jsize};
use redis::Value;
use std::str::FromStr;
use std::sync::Arc;

use std::sync::mpsc;

mod errors;
mod linked_hashmap;

use errors::{FFIError, handle_errors, handle_panics};

#[cfg(ffi_test)]
mod ffi_test;
#[cfg(ffi_test)]
pub use ffi_test::*;

/// Configuration for OpenTelemetry integration in the Java client.
///
/// This struct allows you to configure how telemetry data (traces and metrics) is exported to an OpenTelemetry collector.
/// - `traces`: Optional configuration for exporting trace data. If `None`, trace data will not be exported.
/// - `metrics`: Optional configuration for exporting metrics data. If `None`, metrics data will not be exported.
/// - `flush_interval_ms`: Optional interval in milliseconds between consecutive exports of telemetry data. If `None`, a default value will be used.
///
/// At least one of traces or metrics must be provided.
#[derive(Clone)]
pub struct OpenTelemetryConfig {
    /// Optional configuration for exporting trace data. If `None`, trace data will not be exported.
    pub traces: Option<OpenTelemetryTracesConfig>,
    /// Optional configuration for exporting metrics data. If `None`, metrics data will not be exported.
    pub metrics: Option<OpenTelemetryMetricsConfig>,
    /// Optional interval in milliseconds between consecutive exports of telemetry data. If `None`, the default `DEFAULT_FLUSH_SIGNAL_INTERVAL_MS` will be used.
    pub flush_interval_ms: Option<i64>,
}

/// Configuration for exporting OpenTelemetry traces.
///
/// - `endpoint`: The endpoint to which trace data will be exported. Expected format:
///   - For gRPC: `grpc://host:port`
///   - For HTTP: `http://host:port` or `https://host:port`
///   - For file exporter: `file:///absolute/path/to/folder/file.json`
/// - `sample_percentage`: The percentage of requests to sample and create a span for, used to measure command duration. If `None`, a default value DEFAULT_TRACE_SAMPLE_PERCENTAGE will be used.
///   Note: There is a tradeoff between sampling percentage and performance. Higher sampling percentages will provide more detailed telemetry data but will impact performance.
///   It is recommended to keep this number low (1-5%) in production environments unless you have specific needs for higher sampling rates.
#[derive(Clone)]
pub struct OpenTelemetryTracesConfig {
    /// The endpoint to which trace data will be exported.
    pub endpoint: String,
    /// The percentage of requests to sample and create a span for, used to measure command duration. If `None`, a default value DEFAULT_TRACE_SAMPLE_PERCENTAGE will be used.
    pub sample_percentage: Option<u32>,
}

/// Configuration for exporting OpenTelemetry metrics.
///
/// - `endpoint`: The endpoint to which metrics data will be exported. Expected format:
///   - For gRPC: `grpc://host:port`
///   - For HTTP: `http://host:port` or `https://host:port`
///   - For file exporter: `file:///absolute/path/to/folder/file.json`
#[derive(Clone)]
pub struct OpenTelemetryMetricsConfig {
    /// The endpoint to which metrics data will be exported.
    pub endpoint: String,
}
struct Level(i32);

// TODO: Consider caching method IDs here in a static variable (might need RwLock to mutate)
fn resp_value_to_java<'local>(
    env: &mut JNIEnv<'local>,
    val: Value,
    encoding_utf8: bool,
) -> Result<JObject<'local>, FFIError> {
    match val {
        Value::Nil => Ok(JObject::null()),
        Value::SimpleString(data) => {
            if encoding_utf8 {
                Ok(JObject::from(env.new_string(data)?))
            } else {
                Ok(JObject::from(env.byte_array_from_slice(data.as_bytes())?))
            }
        }
        Value::Okay => Ok(JObject::from(env.new_string("OK")?)),
        Value::Int(num) => Ok(env.new_object("java/lang/Long", "(J)V", &[num.into()])?),
        Value::BulkString(data) => {
            if encoding_utf8 {
                let utf8_str = String::from_utf8(data)?;
                Ok(JObject::from(env.new_string(utf8_str)?))
            } else {
                Ok(JObject::from(env.byte_array_from_slice(&data)?))
            }
        }
        Value::Array(array) => array_to_java_array(env, array, encoding_utf8),
        Value::Map(map) => {
            let linked_hash_map = env.new_object("java/util/LinkedHashMap", "()V", &[])?;

            for (key, value) in map {
                let java_key = resp_value_to_java(env, key, encoding_utf8)?;
                let java_value = resp_value_to_java(env, value, encoding_utf8)?;
                env.call_method(
                    &linked_hash_map,
                    "put",
                    "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                    &[(&java_key).into(), (&java_value).into()],
                )?;
            }

            Ok(linked_hash_map)
        }
        Value::Double(float) => Ok(env.new_object("java/lang/Double", "(D)V", &[float.into()])?),
        Value::Boolean(bool) => Ok(env.new_object("java/lang/Boolean", "(Z)V", &[bool.into()])?),
        Value::VerbatimString { format: _, text } => {
            if encoding_utf8 {
                Ok(JObject::from(env.new_string(text)?))
            } else {
                Ok(JObject::from(env.byte_array_from_slice(text.as_bytes())?))
            }
        }
        Value::BigNumber(_num) => todo!(),
        Value::Set(array) => {
            let set = env.new_object("java/util/HashSet", "()V", &[])?;

            for elem in array {
                let java_value = resp_value_to_java(env, elem, encoding_utf8)?;
                env.call_method(
                    &set,
                    "add",
                    "(Ljava/lang/Object;)Z",
                    &[(&java_value).into()],
                )?;
            }

            Ok(set)
        }
        Value::Attribute {
            data: _,
            attributes: _,
        } => todo!(),
        // Create a java `Map<String, Object>` with two keys:
        //   - "kind" which corresponds to the push type, stored as a `String`
        //   - "values" which corresponds to the array of values received, stored as `Object[]`
        // Only string messages are supported now by Valkey and `redis-rs`.
        Value::Push { kind, data } => {
            let hash_map = env.new_object("java/util/HashMap", "()V", &[])?;

            let kind_str = env.new_string("kind")?;
            let kind_value_str = env.new_string(format!("{kind:?}"))?;

            let _ = env.call_method(
                &hash_map,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[(&kind_str).into(), (&kind_value_str).into()],
            )?;

            let values_str = env.new_string("values")?;
            let values = array_to_java_array(env, data, encoding_utf8)?;

            let _ = env.call_method(
                &hash_map,
                "put",
                "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
                &[(&values_str).into(), (&values).into()],
            )?;

            Ok(hash_map)
        }
        Value::ServerError(server_error) => {
            let err_msg = error_message(&server_error.into());
            let java_exception = env.new_object(
                "glide/api/models/exceptions/RequestException",
                "(Ljava/lang/String;)V",
                &[(&env.new_string(err_msg)?).into()],
            )?;
            Ok(java_exception)
        }
    }
}

/// Convert an array of values into java array of corresponding values.
///
/// Recursively calls to [`resp_value_to_java`] for every element.
///
/// Returns an arbitrary java `Object[]`.
fn array_to_java_array<'local>(
    env: &mut JNIEnv<'local>,
    values: Vec<Value>,
    encoding_utf8: bool,
) -> Result<JObject<'local>, FFIError> {
    let items: JObjectArray =
        env.new_object_array(values.len() as i32, "java/lang/Object", JObject::null())?;

    for (i, item) in values.into_iter().enumerate() {
        let java_value = resp_value_to_java(env, item, encoding_utf8)?;
        env.set_object_array_element(&items, i as i32, java_value)?;
    }

    Ok(items.into())
}

/// # Safety
/// * `pointer` must not be `null`.
/// * `pointer` must be able to be safely casted to a valid [`Box<Value>`] via [`Box::from_raw`]. See the safety documentation of [`Box::from_raw`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_glide_ffi_resolvers_GlideValueResolver_valueFromPointer<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    pointer: jlong,
) -> JObject<'local> {
    handle_panics(
        move || {
            fn value_from_pointer<'a>(
                env: &mut JNIEnv<'a>,
                pointer: jlong,
            ) -> Result<JObject<'a>, FFIError> {
                let value = unsafe { Box::from_raw(pointer as *mut Value) };
                resp_value_to_java(env, *value, true)
            }
            let result = value_from_pointer(&mut env, pointer);
            handle_errors(&mut env, result)
        },
        "valueFromPointer",
    )
    .unwrap_or(JObject::null())
}

/// # Safety
/// * `pointer` must not be `null`.
/// * `pointer` must be able to be safely casted to a valid [`Box<Value>`] via [`Box::from_raw`]. See the safety documentation of [`Box::from_raw`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_glide_ffi_resolvers_GlideValueResolver_valueFromPointerBinary<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    pointer: jlong,
) -> JObject<'local> {
    handle_panics(
        move || {
            fn value_from_pointer_binary<'a>(
                env: &mut JNIEnv<'a>,
                pointer: jlong,
            ) -> Result<JObject<'a>, FFIError> {
                let value = unsafe { Box::from_raw(pointer as *mut Value) };
                resp_value_to_java(env, *value, false)
            }
            let result = value_from_pointer_binary(&mut env, pointer);
            handle_errors(&mut env, result)
        },
        "valueFromPointerBinary",
    )
    .unwrap_or(JObject::null())
}

/// Creates a leaked vector of byte arrays representing the args and returns a handle to it.
///
/// This function is meant to be invoked by Java using JNI.
///
/// * `env`     - The JNI environment.
/// * `_class`  - The class object. Not used.
/// * `args`    - The arguments. This should be a byte[][] from Java.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_GlideValueResolver_createLeakedBytesVec<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    args: JObjectArray<'local>,
) -> jlong {
    handle_panics(
        move || {
            fn create_leaked_bytes_vec<'a>(
                env: &mut JNIEnv<'a>,
                args: JObjectArray<'a>,
            ) -> Result<jlong, FFIError> {
                let num_elements = env.get_array_length(&args)?;
                let mut bytes_vec = Vec::with_capacity(num_elements as usize);

                for index in 0..num_elements {
                    let value = env.get_object_array_element(&args, index as jsize)?;
                    bytes_vec.push(Bytes::from(
                        env.convert_byte_array(JByteArray::from(value))?,
                    ))
                }
                Ok(Box::leak(Box::new(bytes_vec)) as *mut Vec<Bytes> as jlong)
            }
            let result = create_leaked_bytes_vec(&mut env, args);
            handle_errors(&mut env, result)
        },
        "createLeakedBytesVec",
    )
    .unwrap_or(0)
}

/// Returns the maximum total length in bytes of request arguments.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `_env`    - The JNI environment. Not used.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_GlideValueResolver_getMaxRequestArgsLengthInBytes<
    'local,
>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jlong {
    MAX_REQUEST_ARGS_LENGTH_IN_BYTES as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_SocketListenerResolver_startSocketListener<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JObject<'local> {
    handle_panics(
        move || {
            fn start_socket_listener<'a>(env: &mut JNIEnv<'a>) -> Result<JObject<'a>, FFIError> {
                let (tx, rx) = mpsc::channel::<Result<String, String>>();

                start_socket_listener_core(move |socket_path: Result<String, String>| {
                    // Signals that thread has started
                    let _ = tx.send(socket_path);
                });

                // Wait until the thread has started
                let socket_path = rx.recv();

                match socket_path {
                    Ok(Ok(path)) => env
                        .new_string(path)
                        .map(|p| p.into())
                        .map_err(|err| FFIError::Uds(err.to_string())),
                    Ok(Err(error_message)) => Err(FFIError::Uds(error_message)),
                    Err(error) => Err(FFIError::Uds(error.to_string())),
                }
            }
            let result = start_socket_listener(&mut env);
            handle_errors(&mut env, result)
        },
        "startSocketListener",
    )
    .unwrap_or(JObject::null())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ScriptResolver_storeScript<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    code: JByteArray,
) -> JObject<'local> {
    handle_panics(
        move || {
            fn store_script<'a>(
                env: &mut JNIEnv<'a>,
                code: JByteArray,
            ) -> Result<JObject<'a>, FFIError> {
                let code_byte_array = env.convert_byte_array(code)?;
                let hash = glide_core::scripts_container::add_script(&code_byte_array);
                Ok(JObject::from(env.new_string(hash)?))
            }
            let result = store_script(&mut env, code);
            handle_errors(&mut env, result)
        },
        "storeScript",
    )
    .unwrap_or(JObject::null())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ScriptResolver_dropScript<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    hash: JString,
) {
    handle_panics(
        move || {
            fn drop_script(env: &mut JNIEnv<'_>, hash: JString) -> Result<(), FFIError> {
                let hash_str: String = env.get_string(&hash)?.into();
                glide_core::scripts_container::remove_script(&hash_str);
                Ok(())
            }
            let result = drop_script(&mut env, hash);
            handle_errors(&mut env, result)
        },
        "dropScript",
    )
    .unwrap_or(())
}

impl From<logger_core::Level> for Level {
    fn from(level: logger_core::Level) -> Self {
        match level {
            logger_core::Level::Error => Level(0),
            logger_core::Level::Warn => Level(1),
            logger_core::Level::Info => Level(2),
            logger_core::Level::Debug => Level(3),
            logger_core::Level::Trace => Level(4),
            logger_core::Level::Off => Level(5),
        }
    }
}

impl TryFrom<Level> for logger_core::Level {
    type Error = FFIError;
    fn try_from(level: Level) -> Result<Self, <logger_core::Level as TryFrom<Level>>::Error> {
        match level.0 {
            0 => Ok(logger_core::Level::Error),
            1 => Ok(logger_core::Level::Warn),
            2 => Ok(logger_core::Level::Info),
            3 => Ok(logger_core::Level::Debug),
            4 => Ok(logger_core::Level::Trace),
            5 => Ok(logger_core::Level::Off),
            _ => Err(FFIError::Logger(format!(
                "Invalid log level: {:?}",
                level.0
            ))),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_LoggerResolver_logInternal<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    level: jint,
    log_identifier: JString<'local>,
    message: JString<'local>,
) {
    handle_panics(
        move || {
            fn log_internal(
                env: &mut JNIEnv<'_>,
                level: jint,
                log_identifier: JString<'_>,
                message: JString<'_>,
            ) -> Result<(), FFIError> {
                let level = Level(level);

                let log_identifier: String = env.get_string(&log_identifier)?.into();

                let message: String = env.get_string(&message)?.into();

                logger_core::log(level.try_into()?, log_identifier, message);
                Ok(())
            }
            let result = log_internal(&mut env, level, log_identifier, message);
            handle_errors(&mut env, result)
        },
        "logInternal",
    )
    .unwrap_or(())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_LoggerResolver_initInternal<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    level: jint,
    file_name: JString<'local>,
) -> jint {
    handle_panics(
        move || {
            fn init_internal(
                env: &mut JNIEnv<'_>,
                level: jint,
                file_name: JString<'_>,
            ) -> Result<jint, FFIError> {
                let level = if level >= 0 { Some(level) } else { None };
                let file_name: Option<String> = match env.get_string(&file_name) {
                    Ok(file_name) => Some(file_name.into()),
                    Err(JniError::NullPtr(_)) => None,
                    Err(err) => return Err(err.into()),
                };
                let level = match level {
                    Some(lvl) => Some(Level(lvl).try_into()?),
                    None => None,
                };
                let logger_level = logger_core::init(level, file_name.as_deref());
                Ok(Level::from(logger_level).0)
            }
            let result = init_internal(&mut env, level, file_name);
            handle_errors(&mut env, result)
        },
        "initInternal",
    )
    .unwrap_or(0)
}

/// Releases a ClusterScanCursor handle allocated in Rust.
///
/// This function is meant to be invoked by Java using JNI.
///
/// * `_env`    - The JNI environment. Not used.
/// * `_class`  - The class object. Not used.
/// * cursor      - The cursor handle to release.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ClusterScanCursorResolver_releaseNativeCursor<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    cursor: JString<'local>,
) {
    handle_panics(
        move || {
            fn release_native_cursor(
                env: &mut JNIEnv<'_>,
                cursor: JString<'_>,
            ) -> Result<(), FFIError> {
                let cursor_str: String = env.get_string(&cursor)?.into();
                glide_core::cluster_scan_container::remove_scan_state_cursor(cursor_str);
                Ok(())
            }
            let result = release_native_cursor(&mut env, cursor);
            handle_errors(&mut env, result)
        },
        "releaseNativeCursor",
    )
    .unwrap_or(())
}

/// Returns the String representing a finished cursor handle.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ClusterScanCursorResolver_getFinishedCursorHandleConstant<
    'local,
>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_create_jstring(env, FINISHED_SCAN_CURSOR, "getFinishedCursorHandleConstant")
}

/// Returns the String representing the name of the ObjectType String.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ObjectTypeResolver_getTypeStringConstant<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_create_jstring(env, TYPE_STRING, "getTypeStringConstant")
}

/// Returns the String representing the name of the ObjectType List.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ObjectTypeResolver_getTypeListConstant<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_create_jstring(env, TYPE_LIST, "getTypeListConstant")
}

/// Returns the String representing the name of the ObjectType Set.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ObjectTypeResolver_getTypeSetConstant<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_create_jstring(env, TYPE_SET, "getTypeSetConstant")
}

/// Returns the String representing the name of the ObjectType ZSet.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ObjectTypeResolver_getTypeZSetConstant<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_create_jstring(env, TYPE_ZSET, "getTypeZSetConstant")
}

/// Returns the String representing the name of the ObjectType Hash.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ObjectTypeResolver_getTypeHashConstant<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_create_jstring(env, TYPE_HASH, "getTypeHashConstant")
}

/// Returns the String representing the name of the ObjectType Set.
///
/// This function is meant to be invoked by Java using JNI. This is used to ensure
/// that this constant is consistent with the Rust client.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_ObjectTypeResolver_getTypeStreamConstant<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    safe_create_jstring(env, TYPE_STREAM, "getTypeStreamConstant")
}

/// Returns a Java's `HashMap` representing the statistics collected for this process.
///
/// This function is meant to be invoked by Java using JNI.
///
/// * `env`    - The JNI environment.
/// * `_class`  - The class object. Not used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_StatisticsResolver_getStatistics<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JObject<'local> {
    let Some(mut map) = linked_hashmap::new_linked_hashmap(&mut env) else {
        return JObject::null();
    };

    linked_hashmap::put_strings(
        &mut env,
        &mut map,
        "total_connections",
        &format!("{}", Telemetry::total_connections()),
    );

    linked_hashmap::put_strings(
        &mut env,
        &mut map,
        "total_clients",
        &format!("{}", Telemetry::total_clients()),
    );

    map
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_OpenTelemetryResolver_initOpenTelemetry<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    traces_endpoint: JString<'local>,
    traces_sample_percentage: jint,
    metrics_endpoint: JString<'local>,
    flush_interval_ms: jlong,
) -> JObject<'local> {
    handle_panics(
        move || {
            fn init_open_telemetry<'a>(
                env: &mut JNIEnv<'a>,
                traces_endpoint: JString<'a>,
                traces_sample_percentage: jint,
                metrics_endpoint: JString<'a>,
                flush_interval_ms: jlong,
            ) -> Result<JObject<'a>, FFIError> {
                // Convert JString to Rust String or None if null
                let traces_endpoint: Option<String> = match env.get_string(&traces_endpoint) {
                    Ok(endpoint) => Some(endpoint.into()),
                    Err(JniError::NullPtr(_)) => None,
                    Err(err) => return Err(err.into()),
                };

                let metrics_endpoint: Option<String> = match env.get_string(&metrics_endpoint) {
                    Ok(endpoint) => Some(endpoint.into()),
                    Err(JniError::NullPtr(_)) => None,
                    Err(err) => return Err(err.into()),
                };

                // Validate that at least one endpoint is provided
                if traces_endpoint.is_none() && metrics_endpoint.is_none() {
                    return Err(FFIError::OpenTelemetry(
                        "At least one of traces or metrics must be provided for OpenTelemetry configuration.".to_string(),
                    ));
                }
                // Validate flush interval
                if flush_interval_ms <= 0 {
                    return Err(FFIError::OpenTelemetry(format!(
                        "InvalidInput: flushIntervalMs must be a positive integer (got: {flush_interval_ms})"
                    )));
                }

                let mut config = glide_core::GlideOpenTelemetryConfigBuilder::default();

                // Initialize traces exporter if endpoint is provided
                if let Some(endpoint) = traces_endpoint {
                    config = config.with_trace_exporter(
                        glide_core::GlideOpenTelemetrySignalsExporter::from_str(&endpoint)
                            .map_err(|e| FFIError::OpenTelemetry(format!("{e}")))?,
                        if traces_sample_percentage >= 0 {
                            Some(traces_sample_percentage as u32)
                        } else {
                            return Err(FFIError::OpenTelemetry(format!(
                                "InvalidInput: traces_sample_percentage must be a positive integer (got: {traces_sample_percentage})"
                                ))
                            );
                        },
                    );
                }

                // Initialize metrics exporter if endpoint is provided
                if let Some(endpoint) = metrics_endpoint {
                    config = config.with_metrics_exporter(
                        glide_core::GlideOpenTelemetrySignalsExporter::from_str(&endpoint)
                            .map_err(|e| FFIError::OpenTelemetry(format!("{e}")))?,
                    );
                }

                // Set flush interval
                config = config.with_flush_interval(std::time::Duration::from_millis(flush_interval_ms as u64));

                // Initialize OpenTelemetry
                let glide_rt = match glide_core::client::get_or_init_runtime() {
                    Ok(handle) => handle,
                    Err(err) => {
                        return Err(FFIError::OpenTelemetry(format!(
                            "Failed to get or init runtime: {err}"
                        )))
                    }
                };

                glide_rt.runtime.block_on(async {
                    if let Err(e) = glide_core::GlideOpenTelemetry::initialise(config.build()) {
                        logger_core::log(
                            logger_core::Level::Error,
                            "OpenTelemetry",
                            format!("Failed to initialize OpenTelemetry: {e}"),
                        );
                        return Err(FFIError::OpenTelemetry(format!(
                            "Failed to initialize OpenTelemetry: {e}"
                        )));
                    }
                    Ok(())
                })?;

                Ok(JObject::null())
            }
            let result = init_open_telemetry(&mut env, traces_endpoint, traces_sample_percentage, metrics_endpoint, flush_interval_ms);
            handle_errors(&mut env, result)
        },
        "initOpenTelemetry",
    )
    .unwrap_or(JObject::null())
}

/// Creates an open telemetry span with the given name and returns a pointer to the span
#[unsafe(no_mangle)]
pub extern "system" fn Java_glide_ffi_resolvers_OpenTelemetryResolver_createLeakedOtelSpan<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    name: JString<'local>,
) -> jlong {
    handle_panics(
        move || {
            fn create_leaked_otel_span<'a>(
                env: &mut JNIEnv<'a>,
                name: JString<'a>,
            ) -> Result<jlong, FFIError> {
                let name_str: String = env.get_string(&name)?.into();
                let span = glide_core::GlideOpenTelemetry::new_span(&name_str);
                let s = Arc::into_raw(Arc::new(span)) as *mut glide_core::GlideSpan;
                Ok(s as jlong)
            }
            let result = create_leaked_otel_span(&mut env, name);
            handle_errors(&mut env, result)
        },
        "createLeakedOtelSpan",
    )
    .unwrap_or(0)
}

/// Drops an OpenTelemetry span given its pointer
/// # Safety
/// * `span_ptr` must not be `null`.
/// * `span_ptr` must be able to be safely casted to a valid [`Arc<glide_core::GlideSpan>`] via [`Arc::from_raw`]. See the safety documentation of [`Arc::from_raw`].
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_glide_ffi_resolvers_OpenTelemetryResolver_dropOtelSpan<
    'local,
>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    span_ptr: jlong,
) {
    handle_panics(
        move || {
            fn drop_otel_span(span_ptr: jlong) -> Result<(), FFIError> {
                if span_ptr <= 0 {
                    return Err(FFIError::OpenTelemetry(
                        "Received an invalid pointer value.".to_string(),
                    ));
                }
                unsafe {
                    Arc::from_raw(span_ptr as *const glide_core::GlideSpan);
                }
                Ok(())
            }
            let result = drop_otel_span(span_ptr);
            handle_errors(&mut env, result)
        },
        "dropOtelSpan",
    )
    .unwrap_or(())
}

/// Convert a Rust string to a Java String and handle errors.
///
/// * `env`             - The JNI environment.
/// * `_class`          - The class object. Not used.
/// * `input`           - The String to convert.
/// * `functionName`    - The name of the calling function.
fn safe_create_jstring<'local>(
    mut env: JNIEnv<'local>,
    input: &str,
    function_name: &str,
) -> JString<'local> {
    handle_panics(
        move || {
            fn create_jstring<'a>(
                env: &mut JNIEnv<'a>,
                input: &str,
            ) -> Result<JString<'a>, FFIError> {
                Ok(env.new_string(input)?)
            }
            let result = create_jstring(&mut env, input);
            handle_errors(&mut env, result)
        },
        function_name,
    )
    .unwrap_or(JString::<'_>::default())
}
