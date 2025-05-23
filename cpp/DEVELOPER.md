## Developer Guide

This document describes how to set up your development environment to build and test the Valkey GLIDE C++ wrapper.

### Development Overview

We're excited to share that the GLIDE Go client is currently in development! However, it's important to note that this client is a work in progress and is not yet complete or fully tested.
Your contributions and feedback are highly encouraged as we work towards refining and improving this implementation.
Thank you for your interest and understanding as we continue to develop this C++ wrapper.

The Valkey GLIDE C++ wrapper consists of both C++ and Rust code. The C++ and Rust components communicate in two ways:
1. Using the [protobuf](https://github.com/protocolbuffers/protobuf) protocol to establish connection.
2. Utilizing shared C++ objects generated by cbindgen.

### Build from source

#### Prerequisites

Software Dependencies

-    gcc
-    cmake
-    openssl
-    protobuf >= v3.20.0
-    abseil-cpp
-    pkg-config
-    python (required for benchmarking)
-    doxygen (required for generating documentation)

A `shell.nix` file is available to swiftly install the necessary dependencies.

**Valkey installation**

To install valkey-server and valkey-cli on your host, follow the [Valkey Installation Guide](https://github.com/valkey-io/valkey).

#### Building and installation steps

Before starting this step, make sure you've installed all software requirements.

1. Clone the repository:
    ```bash
    VERSION=0.1.0 # You can modify this to other released version or set it to "main" to get the unstable branch
    git clone --branch ${VERSION} https://github.com/valkey-io/valkey-glide.git
    cd valkey-glide
    ```
2. Initialize git submodule:
    ```bash
    git submodule update --init --recursive
    ```
3. Install build dependencies:
    ```bash
    cd cpp
    mkdir build && cd build
    cmake .. -DDEBUG_MODE=ON
    make generate-proto
    make generate-cbinding
    export GLIDE_VERSION="dev"
    export GLIDE_NAME="glide"
    make prebuild
    ```
5. Build the C++ wrapper:
    ```bash
    make -j8
    ```

### Test

To run tests, use the following command:

```bash
    cd cpp/build/test
    ctest --output-on-failure
```

For memory check, use the following command:

You must first build the sample code before running Valgrind on it.
```bash
cd examples/
mkdir build && cd build
cmake .. -DDEBUG_MODE=ON
make -j8
valgrind --leak-check=full --show-leak-kinds=all --track-origins=yes ./glide-cpp-sample
```

### Submodules

After pulling new changes, ensure that you update the submodules by running the following command:

```bash
git submodule update
```

### Generate protobuf files

If modifications are made to the protobuf definition files (.proto files located in `glide-core/src/protobuf`), it becomes necessary to regenerate the Go protobuf files. To do so, run:

```bash
make generate-proto
```

#### Language-specific Linters

-   valgrind

#### Running the linters

Run from the main `/cpp` folder

```bash
make lint
```

### Benchmarks

To run the benchmarks, ensure you have followed the [build and installation steps](#building-and-installation-steps) (the tests do not have to be run). Then execute the following:

```bash
cd cpp/benchmarks
python benchmarks.py
```

### Generating documentation

```bash
cd docs
git clone https://github.com/jothepro/doxygen-awesome-css.git
doxygen Doxyfile
```

You'll get the generated documentation in the `output/html` folder.

### Recommended extensions for VS Code

-   [C++](https://marketplace.visualstudio.com/items?itemName=ms-vscode.cpptools)
-   [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
