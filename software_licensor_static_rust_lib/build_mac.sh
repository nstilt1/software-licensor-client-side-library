# Add the mac targets
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin

# Build for both mac targets
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

mkdir target/universal
# combine the builds into a universal library
lipo -create -o target/universal/libsoftwarelicensor.a \
    target/aarch64-apple-darwin/release/libsoftwarelicensor.a \
    target/x86_64-apple-darwin/release/libsoftwarelicensor.a