# Software Licensor Client Side Library

This repo contains the client side code for the Software Licensor API.

# Status

The static Rust library currently seems to work when called from C++. I still need to make a JUCE library that provides a GUI for the user, as well as providing some inputs for the developer for their store ID and whatnot.

# Building

The Rust static library can be built with `cargo build --release`.

Then you might need to run `cmake .` in `cpp_test/`, followed by `make`.