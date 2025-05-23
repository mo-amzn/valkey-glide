/** Copyright Valkey GLIDE Project Contributors - SPDX Identifier: Apache-2.0 */
package glide.api.models;

import static command_request.CommandRequestOuterClass.RequestType.Copy;
import static command_request.CommandRequestOuterClass.RequestType.Move;
import static command_request.CommandRequestOuterClass.RequestType.Scan;
import static command_request.CommandRequestOuterClass.RequestType.Select;
import static glide.api.commands.GenericBaseCommands.REPLACE_VALKEY_API;
import static glide.api.commands.GenericCommands.DB_VALKEY_API;
import static glide.utils.ArgsBuilder.checkTypeOrThrow;
import static glide.utils.ArgsBuilder.newArgsBuilder;

import glide.api.GlideClient;
import glide.api.models.commands.scan.ScanOptions;
import lombok.NonNull;

/**
 * Batch implementation for standalone {@link GlideClient}. Batches allow the execution of a group
 * of commands in a single step.
 *
 * <p>Batch Response: An <code>array</code> of command responses is returned by the client {@link
 * GlideClient#exec} API, in the order they were given. Each element in the array represents a
 * command given to the {@link Batch}. The response for each command depends on the executed Valkey
 * command. Specific response types are documented alongside each method.
 *
 * <p><strong>isAtomic:</strong> Determines whether the batch is atomic or non-atomic. If {@code
 * true}, the batch will be executed as an atomic transaction. If {@code false}, the batch will be
 * executed as a non-atomic pipeline.
 *
 * @see <a href="https://valkey.io/docs/topics/transactions/">Valkey Transactions (Atomic
 *     Batches)</a>
 * @see <a href="https://valkey.io/topics/pipelining">Valkey Pipelines (Non-Atomic Batches)</a>
 * @remarks Standalone Batches are executed on the primary node.
 * @example
 *     <pre>{@code
 * // Example of Atomic Batch (Transaction)
 * Batch transaction = new Batch(true) // Atomic (Transactional)
 *     .set("key", "value")
 *     .get("key");
 * Object[] result = client.exec(transaction, false).get();
 * // result contains: OK and "value"
 * assert result[0].equals("OK");
 * assert result[1].equals("value");
 * }</pre>
 *
 * @example
 *     <pre>{@code
 * // Example of Non-Atomic Batch (Pipeline)
 * Batch pipeline = new Batch(false) // Non-Atomic (Pipeline)
 *     .set("key1", "value1")
 *     .set("key2", "value2")
 *     .get("key1")
 *     .get("key2");
 * Object[] result = client.exec(pipeline, false).get();
 * // result contains: OK, OK, "value1", "value2"
 * assert result[0].equals("OK");
 * assert result[1].equals("OK");
 * assert result[2].equals("value1");
 * assert result[3].equals("value2");
 * }</pre>
 */
public class Batch extends BaseBatch<Batch> {

    /**
     * Creates a new Batch instance.
     *
     * @param isAtomic Determines whether the batch is atomic or non-atomic. If {@code true}, the
     *     batch will be executed as an atomic transaction. If {@code false}, the batch will be
     *     executed as a non-atomic pipeline.
     */
    public Batch(boolean isAtomic) {
        super(isAtomic);
    }

    @Override
    protected Batch getThis() {
        return this;
    }

    /**
     * Changes the currently selected server database.
     *
     * @see <a href="https://valkey.io/commands/select/">valkey.io</a> for details.
     * @param index The index of the database to select.
     * @return Command Response - A simple <code>OK</code> response.
     */
    public Batch select(long index) {
        protobufBatch.addCommands(buildCommand(Select, newArgsBuilder().add(index)));
        return this;
    }

    /**
     * Move <code>key</code> from the currently selected database to the database specified by <code>
     * dbIndex</code>.
     *
     * @implNote {@link ArgType} is limited to {@link String} or {@link GlideString}, any other type
     *     will throw {@link IllegalArgumentException}.
     * @see <a href="https://valkey.io/commands/move/">valkey.io</a> for more details.
     * @param key The key to move.
     * @param dbIndex The index of the database to move <code>key</code> to.
     * @return Command Response - <code>true</code> if <code>key</code> was moved, or <code>false
     *     </code> if the <code>key</code> already exists in the destination database or does not
     *     exist in the source database.
     */
    public <ArgType> Batch move(ArgType key, long dbIndex) {
        checkTypeOrThrow(key);
        protobufBatch.addCommands(buildCommand(Move, newArgsBuilder().add(key).add(dbIndex)));
        return this;
    }

    /**
     * Copies the value stored at the <code>source</code> to the <code>destination</code> key on
     * <code>destinationDB</code>. When <code>replace</code> is true, removes the <code>destination
     * </code> key first if it already exists, otherwise performs no action.
     *
     * @since Valkey 6.2.0 and above.
     * @implNote {@link ArgType} is limited to {@link String} or {@link GlideString}, any other type
     *     will throw {@link IllegalArgumentException}.
     * @see <a href="https://valkey.io/commands/copy/">valkey.io</a> for details.
     * @param source The key to the source value.
     * @param destination The key where the value should be copied to.
     * @param destinationDB The alternative logical database index for the destination key.
     * @return Command Response - <code>true</code> if <code>source</code> was copied, <code>false
     *     </code> if <code>source</code> was not copied.
     */
    public <ArgType> Batch copy(
            @NonNull ArgType source, @NonNull ArgType destination, long destinationDB) {
        return copy(source, destination, destinationDB, false);
    }

    /**
     * Copies the value stored at the <code>source</code> to the <code>destination</code> key on
     * <code>destinationDB</code>. When <code>replace</code> is true, removes the <code>destination
     * </code> key first if it already exists, otherwise performs no action.
     *
     * @since Valkey 6.2.0 and above.
     * @implNote {@link ArgType} is limited to {@link String} or {@link GlideString}, any other type
     *     will throw {@link IllegalArgumentException}.
     * @see <a href="https://valkey.io/commands/copy/">valkey.io</a> for details.
     * @param source The key to the source value.
     * @param destination The key where the value should be copied to.
     * @param destinationDB The alternative logical database index for the destination key.
     * @param replace If the destination key should be removed before copying the value to it.
     * @return Command Response - <code>true</code> if <code>source</code> was copied, <code>false
     *     </code> if <code>source</code> was not copied.
     */
    public <ArgType> Batch copy(
            @NonNull ArgType source, @NonNull ArgType destination, long destinationDB, boolean replace) {
        checkTypeOrThrow(source);
        protobufBatch.addCommands(
                buildCommand(
                        Copy,
                        newArgsBuilder()
                                .add(source)
                                .add(destination)
                                .add(DB_VALKEY_API)
                                .add(destinationDB)
                                .addIf(REPLACE_VALKEY_API, replace)));
        return this;
    }

    /**
     * Iterates incrementally over a database for matching keys.
     *
     * @see <a href="https://valkey.io/commands/scan">valkey.io</a> for details.
     * @param cursor The cursor that points to the next iteration of results. A value of <code>"0"
     *     </code> indicates the start of the search.
     * @return Command Response - An <code>Array</code> of <code>Objects</code>. The first element is
     *     always the <code>cursor</code> for the next iteration of results. <code>"0"</code> will be
     *     the <code>cursor</code> returned on the last iteration of the scan.<br>
     *     The second element is always an <code>Array</code> of matched keys from the database.
     */
    public <ArgType> Batch scan(@NonNull ArgType cursor) {
        checkTypeOrThrow(cursor);
        protobufBatch.addCommands(buildCommand(Scan, newArgsBuilder().add(cursor)));
        return this;
    }

    /**
     * Iterates incrementally over a database for matching keys.
     *
     * @see <a href="https://valkey.io/commands/scan">valkey.io</a> for details.
     * @param cursor The cursor that points to the next iteration of results. A value of <code>"0"
     *     </code> indicates the start of the search.
     * @param options The {@link ScanOptions}.
     * @return Command Response - An <code>Array</code> of <code>Objects</code>. The first element is
     *     always the <code>cursor</code> for the next iteration of results. <code>"0"</code> will be
     *     the <code>cursor</code> returned on the last iteration of the scan.<br>
     *     The second element is always an <code>Array</code> of matched keys from the database.
     */
    public <ArgType> Batch scan(@NonNull ArgType cursor, @NonNull ScanOptions options) {
        checkTypeOrThrow(cursor);
        protobufBatch.addCommands(
                buildCommand(Scan, newArgsBuilder().add(cursor).add(options.toArgs())));
        return this;
    }
}
