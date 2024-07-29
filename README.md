# Software Licensor Client Side Library

This repo contains the client side code for the Software Licensor API.

# Status

The static Rust library currently seems to work when called from C++. I still need to make a JUCE library that provides a GUI for the user, as well as providing some inputs for the developer for their store ID and whatnot.

## Potential Issues with the JUCE code

There are a few potential issues to look out for when compiling and running the JUCE code.

1) the Rust library may not be fully linked. You must build `software_licensor_static_rust_lib` with `cargo build --release` and then provide the path to the library to your IDE or CMake.
2) The thread might timeout after 100 ms while receiving the result from the webserver when unlocking. This might be caused by the `startTimer(100)` call in `run()` in `SoftwareLicensorUnlockForm.cpp`. The preceding function call should take a minimum of 5 seconds to return, so increasing the value to `startTimer(7000)` should suffice. It mainly depends on how the original `juce_OnlineUnlockForm` code works.
3) The message translations may not work properly if the `Texts_XX.properties` files are not found. These files are supposed to translate the text in `SoftwareLicensorStatus.h > getMessage()`. This function is virtual if you have a better way of translating messages, and you can change the messages in the properties files if you want it to say something different.
4) The spacing/layout of the `SoftwareLicensorUnlockForm` may be off.

## Potential issues with the rust code

There is one main potential issue with the Rust code that can be changed. In `file_io.rs`, there's a function called `get_license_file_path()` that returns a file path for the license file. On Linux, this file path may be user-specific. Also, the compiled program will need to have permissions to write to and read from these file paths.

# Building

The Rust static library can be built with `cargo build --release`.

For the cpp_test folder, you might need to run `cmake .` in `cpp_test/`, followed by `make`.

To build the JUCE code, the C++ headers and .cpp files will need to be included in a JUCE plugin project, along with the compiled Rust Library file in `software_licensor_static_rust_lib/target/release`. It might be best to compile the plugin using CMake. Don't forget to rebuild the Rust library for each platform, or cross compile the Rust library for each platform.

# Dependencies

The JUCE code depends on `JUCE-8.0.0`, but it may work on a lower version such as `JUCE-7.0.5`. However, JUCE fixed some issues regarding local machine IDs changing when they weren't supposed to.

This also depends on the following JUCE `modules`:

* `juce_core`
* `juce_cryptography`
* `juce_data_structures`
* `juce_events`
* `juce_graphics`
* `juce_gui_basics`
* `juce_gui_extra`
* `juce_product_unlocking`

# The Licensing License

The rust code is dual-licensed under MIT and Apache, and `SoftwareLicensorJUCE` is licensed under AGPLv3.