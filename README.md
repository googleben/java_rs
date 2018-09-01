This is a project for working with Java from Rust code. Currently includes read-only classfile browsing from inside Rust code as well as a GUI. Hopefully one day will include a JVM.

# Build

Clone the repository and open up the base directory in a shell of your choice and make sure Rust is installed. Then run a command:
## Build only

`cargo build --release`

## Run browser GUI

`cargo run --release -p class_browser`

Binaries are found in `./target/release/`
