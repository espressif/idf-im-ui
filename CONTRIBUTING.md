# Contributing

...
## Running Development Environment
To run the app for development purposes, navigate to your project root directory in your terminal and run `cargo tauri dev` command. 

to run the `CLI only` part use `cargo tauri dev -- --no-default-features --features cli`

for the library part tests navigate to the `src-tauri` directory and run `cargo test --no-default-features --no-fail-fast --lib `