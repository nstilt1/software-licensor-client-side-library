# Software Licensor Client Side Library

This repo contains the client side code for the Software Licensor API.

# Status

The static Rust library currently seems to work when called from C++. The JUCE code also seems to work on Windows and MacOS.

## Compatibility

The main issue with compatibility arises when dealing with where the license file is stored. For some operating systems, you might need to adjust the installer to grant the application read/write permissions on a specific directory. In some operating systems, the path maybe should include `com.companyName.package` or something similar for a directory name in these paths, rather than just calling the directory `CompanyName`.

Below, there are some general notes about our compatibility with different operating systems.

### Windows ✅

This library is compatible with Windows. When saving license files, the default location is a system-wide location at `C:\ProgramData\[company name]\license.bin`. If you get an `IOError` when your application is trying to read and write from here, then you will need to either update the permissions of the application, or change the path in the code. This path is set in `file_io.rs` in `get_license_file_path()`.

I've tested with `cpp_test` on a Windows 11 standard user account, making sure that the folder `C:\ProgramData\SoftwareLicensorTestCompany` did not already exist, and the test was successful.

### MacOS ✅

This library is compatible with MacOS. It will try to write to the system-wide location at `/Library/Application Support/[company name]/license.bin`. If the app is lacking permissions, it will write to a user-specific location at `~/Library/Application Support/[company name]/license.bin`

### Linux ✅

This library is compatible with Linux. It will try to write to a user-specific location at `$HOME/.local/share/[company name]/license.bin`. It could be modified to try to write to a system-wide location, but the program would need permissions to read/write from/to that location.

### Android ❌

This library is probably not compatible with Android. Right now, there is an attempt to write the license file to `/data/data/[company name]/files/license.bin`, but your application will probably need permissions to write there, and I'm not sure what a better directory will be, as I am not an Android developer. The directory name might need to be a package name rather than just a company name.

### iOS ❌

There is no compatibility with iOS at the moment. It seems that in order to save a license file, the path might need to be passed in to Rust... again, I am not an iOS developer.

# Building

## Building the Rust static library

The Rust static library can be built with 

```shell
cargo build --release --target your-target
```

For MacOS, you can call `sudo chmod +x ./build_mac.sh` and then run that script. It will compile the Rust code into x86_64 and aarch64 libraries, and then combined those compiled files into a single file in `software_licensor_static_rust_lib/target/universal`.

For Windows on x86_64, the target should be `x86_64-pc-windows-msvc`.

For the cpp_test folder, you might need to run `cmake .` in `cpp_test/`, followed by `make`.

## Building and Configuring the JUCE code

To build the JUCE code, the C++ headers and .cpp files will need to be included in a JUCE plugin project, along with the compiled Rust Library file in `software_licensor_static_rust_lib/target/release`. It might be best to compile the plugin using CMake. Don't forget to rebuild the Rust library for each platform, or cross compile the Rust library for each platform.

You will need to override the following members of the `SoftwareLicensorStatus` class:
* `getStoreId()` - this is provided in the WordPress Plugin under `Software Licensor` in the admin menu
* `getCompanyName()` - this is only for making a directory in the end user's machine. See `get_license_file_path()` in `file_io.rs`.
* `getProductIdsAndPubkeys()` - a vector of product IDs and their public keys separated by a semicolon. You could define multiple IDs and Public Keys for a piece of software if it is included in a bundle, as well as distributed individually. The product IDs and public keys can be found on the same page that your `storeId` was found on.

### Example

```cpp
class MyLicensingStatus : public SoftwareLicensorStatus
{
public:
    juce::String getStoreId() override
    {
        return juce::String("MyStoreID");
    }

    juce::String getCompanyName() override
    {
        return juce::String("MyCompanyName");
    }

    std::vector<juce::String> getProductIdsAndPubkeys() override
    {
        std::vector<juce::String> productIdsAndPubkeys;

        productIdsAndPubkeys.push_back("MyProductID;Base64PublicKey==");
        productIdsAndPubkeys.push_back("MyBundleProductID;Base64PublicKey==");
        return productIdsAndPubkeys;
    }
};
```

You may wish to make a user module that has overridden `getStoreId()` and `getCompanyName()`, so that way all of your software will use the same directory and store ID.

Then in the `pluginProcessor.h` constructor, you will want to call `unlockStatus.check_license_with_potential_api_request` to initialize the licensing status. There needs to be occasional API requests to renew the license's expiration so that machines can be removed from the license via the store's `Regenerate License` button.

Elsewhere in the code, you can call `unlockStatus.check_license_with_no_api_request()` which will update the result of `unlockStatus.isUnlocked()` based on the locally stored license key file, which contains a product-specific signature from the server. This signature could be altered, but it has 192 bits of security, making the signature unlikely to be cracked directly.

Finally, when you want to check if the plugin is unlocked, you can call `unlockStatus.isUnlocked()`. The other functions will also return this value, but you might not want to call them within the `processBlock` because they both initiate file reads and may take some time to complete.

### Building with Visual Studio 2022 on Windows

For Visual Studio 2022 on Windows, you will need to

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

### Building with XCode on MacOS

Build the rust static library using `software_licensor_static_rust_lib/build_mac.sh`.

For XCode on MacOS... first, if you happen to be using JUCE 7.0.7, you'll need to download both version 7.0.7 and 7.0.8 of JUCE. Build the Projucer from 7.0.8 and use that with JUCE 7.0.7 for setting the source files, and then export the project to XCode. The reason for this is that there's an issue with the XCode project generation in JUCE 7.0.7 that might cause some build errors. Then,

1) Open the project navigator by clicking the folder looking icon towards the top left corner of the window, in the left-most tab.
2) Select your project with the blue icon to the left of it.
3) Choose `File>Add Files to "Your Project Name"`
4) Ensure that the proper targets are selected before adding this file, then locate the universal library file in `software_licensor_static_rust_lib/target/universal` and select it, or select its parent folder if you are unable to select the file itself.
5) Select a desired scheme in `Product>Scheme` and build it.

If there are any issues with the build, consider adding the path to `software_licensor_static_rust_lib/target/universal` into `Projucer>Exporters>XCode>Debug/Release>Extra Library Search Paths`.

#### Signing on MacOS

First, you need to take note of the `entitltements.plist` file in this repo. This file is required for code signing as it allows the application to make API requests with the serverless Software Licensor API. It also allows creation and modification of the license key file in `~/Library/[Vendor]/license.bin` and the hardware info file in `~/Library/com.Hyperformance-Solutions.Software-Licensor/hwinfo.bin`. The `hwinfo.bin` file is conditionally populated with the user's hardware information so that it can be sent to the Software Licensor API. The entitlements also include a "user-selected" file access in the event that the plugin's presets need any permissions to be saved/loaded and if the plugin is lacking permissions.

If you get `Command PhaseScriptExecution failed with a nonzero exit code` when trying to build a signed AU and VST3... this could be for many different reasons, but I deleted the MacOS builds folder, and re-exported the project from Projucer, and I then **left the Code Signing settings under `Build Settings` alone**. I was trying to select the code signing certificate in those `Build Settings` under `Signing`, but it stopped working and cost me several hours to figure out why the builds weren't working. If you're having this issue, consider manually signing the using the command line. Here is a sample command:

```sh
codesign --sign "Developer ID Application: [name] [ID] *OR* SHA-1 hash" \ 
"/path/to/My Plugin.component" \
"/path/to/My Plugin.vst3" \
--timestamp --force --options runtime --verbose --deep \
--entitlements "/path/to/software-licensor-client-side-library/entitlements.plist"
```

# Potential Issues with the JUCE code

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