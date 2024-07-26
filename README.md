# Software Licensor Client Side Library

This repo contains the client side code for the Software Licensor API.

# Status

The static Rust library currently seems to work when called from C++. I still need to make a JUCE library that provides a GUI for the user, as well as providing some inputs for the developer for their store ID and whatnot.

## Potential Issues with the JUCE code

There are two potential issues to look out for when compiling the JUCE code.

1) the Rust library may not be fully linked. You must build `software_licensor_static_rust_lib` with `cargo build --release` and then provide the path to the library to your IDE or CMake.
2) The thread might timeout after 100 ms while receiving the result from the webserver when unlocking. This might be caused by the `startTimer(100)` call in `run()` in `SoftwareLicensorUnlockForm.cpp`. The preceding function call should take a minimum of 5 seconds to return, so increasing the value to `startTimer(7000)` should suffice. It mainly depends on how the original `juce_OnlineUnlockForm` code works.

# Building

The Rust static library can be built with `cargo build --release`.

For the cpp_test folder, you might need to run `cmake .` in `cpp_test/`, followed by `make`.

The C++ headers and .cpp files will need to be included in a JUCE plugin project, along with the compiled Rust Library file in `software_licensor_static_rust_lib/target/release`. It might be best to compile the plugin using CMake. Don't forget to rebuild the Rust library for each platform, or cross compile the Rust library for each platform.

# The Licensing License

The rust code is dual-licensed under MIT and Apache, and `SoftwareLicensorJUCE` is licensed under AGPLv3.