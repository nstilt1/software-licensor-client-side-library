cmake_minimum_required(VERSION 3.10)
project(CppTest)

# Set C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Include directories for header files
include_directories(include)

# Add executable
add_executable(cpp_test main.cpp)

# Link against the Rust library
if(WIN32)
    target_link_libraries(cpp_test 
        ${CMAKE_SOURCE_DIR}/../software_licensor_static_rust_lib/target/x86_64-pc-windows-msvc/release/software_licensor_static_rust_lib.lib
        Userenv
        Ntdll
        Bcrypt
        Ws2_32
    )
else()
    target_link_libraries(cpp_test ${CMAKE_SOURCE_DIR}/../software_licensor_static_rust_lib/target/universal/libsoftwarelicensor.a)
endif()
# Assuming the Rust library outputs to the target/release directory

# Include pthread if using std::thread or other threading features in C++
find_package(Threads)
target_link_libraries(cpp_test ${CMAKE_THREAD_LIBS_INIT})