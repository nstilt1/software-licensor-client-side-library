# Add the mac targets
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin

if [ -z "$1" ]; then
    echo "compiling without extra features"
    features_flag=""
else
    echo "compiling with features: $1"
    features_flag="--features $1"
fi

# Build for both mac targets
cargo build --release ${features_flag} --target aarch64-apple-darwin
cargo build --release ${features_flag} --target x86_64-apple-darwin

mkdir target/universal
# combine the builds into a universal library
cd ..
mkdir target/universal
rm -rf target/universal/libsoftwarelicensor.a
lipo -create -o target/universal/libsoftwarelicensor.a \
    target/aarch64-apple-darwin/release/libsoftwarelicensor.a \
    target/x86_64-apple-darwin/release/libsoftwarelicensor.a