# Software Licensor Client Side Library

This repo contains the client side code for the Software Licensor API.

# Status

The static Rust library currently seems to work when called from C++. The JUCE code also seems to work on Windows. I have not tried building it on MacOS yet.

# Building

The Rust static library can be built with 

```shell
cargo build --release --target your-target
```

For Windows on x86_64, the target should be `x86_64-pc-windows-msvc`.

For the cpp_test folder, you might need to run `cmake .` in `cpp_test/`, followed by `make`.

To build the JUCE code, the C++ headers and .cpp files will need to be included in a JUCE plugin project, along with the compiled Rust Library file in `software_licensor_static_rust_lib/target/release`. It might be best to compile the plugin using CMake. Don't forget to rebuild the Rust library for each platform, or cross compile the Rust library for each platform.

You will need to override the following members of the `SoftwareLicensorStatus` class:
* `getStoreId()` - this is provided in the WordPress Plugin under `Software Licensor` in the admin menu
* `getCompanyName()` - this is only for making a directory in the end user's machine. See `get_license_file_path()` in `file_io.rs`.
* `getProductIdsAndPubkeys()` - a vector of product IDs and their public keys separated by a semicolon. You could define multiple IDs and Public Keys for a piece of software if it is included in a bundle, as well as distributed individually. The product IDs and public keys can be found on the same page that your `storeId` was found on.

* For Visual Studio 2022 on Windows, you will need to
  1) Right-click the project in the IDE. In my instance it was the VST3 project in the IDE.
  2) Ensure that it is set up as the startup project.
  3) Select `Properties`, then select the build configuration in the top left. You might want to choose Debug if you need to debug.
  4) Select `Linker>Input>Additional Dependencies`, then add these on separate lines:
    * Userenv.lib
    * Ntdll.lib
    * software_licensor_static_rust_lib.lib
    * Bcrypt.lib
    * Ws2_32.lib
  5) Select `Linker>General>Additional Library Directories` and ensure that the path to the client side Rust library is there, ending with `software-licensor-client-side-library\software_licensor_static_rust_lib\target\x86_64-pc-windows-msvc\release`
  6) Apply the changes and build in the configuration that you specified.

## Potential Issues with the JUCE code

There are a few potential issues to look out for when compiling and running the JUCE code.

1) the Rust library may not be fully linked. You must build `software_licensor_static_rust_lib` with `cargo build --release` and then provide the path to the library to your IDE or CMake, as well as the library's name.
  
2) The spacing/layout of the `SoftwareLicensorUnlockForm` may be off. You are able to change this to your liking.

## Potential issues with the rust code

There is one main potential issue with the Rust code that can be changed. In `file_io.rs`, there's a function called `get_license_file_path()` that returns a file path for the license file. On Linux, this file path may be user-specific. Also, the compiled program will need to have permissions to write to and read from these file paths.

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

There may also be some dependencies for building on Windows, but you shouldn't need to download them on Windows:

* `Userenv.lib`
* `Ntdll.lib`
* `Bcrypt.lib`
* `Ws2_32.lib`

# The Licensing License

The rust code is dual-licensed under MIT and Apache, and `SoftwareLicensorJUCE` is licensed under AGPLv3.