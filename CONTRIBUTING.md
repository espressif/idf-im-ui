# Contributing Guide

Thank you for your interest in contributing to EIM! This guide will help you get started with development and ensure your contributions meet the project's standards.

## Project Overview

This application is built using Tauri 2.0 and offers both CLI and GUI capabilities. The project structure consists of:

- `./src-tauri/`: Contains the Rust backend code (library and CLI)
- Root directory: Contains the frontend code and Tauri configuration

## Development Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) and Cargo
- [Node.js](https://nodejs.org/) and npm/yarn and vite
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/setup/)
- [Perl](https://www.perl.org/get.html) is probably needed on windows to build the openssl library

## Important Note About Project Management

**Important:** You must use `cargo` for project management rather than `yarn tauri ...` commands, as the latter does not support passing arguments to underlying processes.

## Building and Running the Project

### Library Component

To modify and build the library component:

```bash
cd ./src-tauri/
cargo build --release --no-default-features --lib
```

To run the library tests:

```bash
cd ./src-tauri/
cargo test --no-default-features --no-fail-fast --lib
```

### CLI Component

To build the CLI application:

```bash
cd ./src-tauri/
cargo build --release --no-default-features --features cli
```

To run the CLI application:

```bash
cd ./src-tauri/
cargo run --release --no-default-features --features cli
```

You can pass parameters to the CLI application by adding them after a double dash:

```bash
cargo run --release --no-default-features --features cli -- --help
```

### GUI Component

To run the GUI application in development mode:

```bash
# From the root of the repository
cargo tauri dev
```

To pass CLI arguments to the GUI binary:

```bash
cargo tauri dev -- -- --help
```

Note the double `-- --` syntax: the first `--` separates the Cargo arguments from the Tauri arguments, and the second `--` separates the Tauri arguments from the application arguments.

## Cross-Platform Compatibility

All contributions **must** maintain multi-platform compatibility. While platform-specific enhancements are welcome, they cannot break functionality on other platforms:

- If you add Windows-specific features, ensure they don't break Linux or macOS functionality
- If you add macOS-specific features, ensure they don't break Windows or Linux functionality
- If you add Linux-specific features, ensure they don't break Windows or macOS functionality

When implementing platform-specific code, use runtime checks with `std::env::consts::OS` rather than compile-time conditional compilation:

```rust
fn platform_specific_function() {
    match std::env::consts::OS {
        "windows" => {
            // Windows-specific code
        }
        "macos" => {
            // macOS-specific code
        }
        "linux" => {
            // Linux-specific code
        }
        _ => {
            // Default behavior for other platforms
        }
    }
}
```

This approach is preferred over using `#[cfg(target_os = "...")]` macros because:

1. Code written this way is not skipped by the linter and parser on different platforms
2. It's harder to break dependencies across platforms
3. It provides better visibility of cross-platform issues during development
4. It ensures all code paths are properly tested

While this may result in slightly larger binaries, the improved maintainability and cross-platform stability are worth the trade-off.

## Pull Request Process

1. Fork the repository and create a feature branch
2. Implement your changes
3. Add tests for your changes
4. Ensure all tests pass on all supported platforms
5. Update documentation as necessary
6. Submit a pull request

## Code Style

- Follow Rust's official style guidelines
- Run `cargo fmt` before submitting code
- Ensure your code passes `cargo clippy` without warnings

## License

By contributing to this project, you agree that your contributions will be licensed under the project's license.

Thank you for helping improve our application!