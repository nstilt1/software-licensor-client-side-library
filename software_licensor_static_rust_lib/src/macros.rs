/// Intializes a tokio::runtime::Runtime
#[macro_export]
macro_rules! runtime {
    (true) => {
        match Runtime::new() {
            Ok(v) => v,
            Err(_) => {
                return box_out!(LicenseData::error("There was an error starting a runtime"))
            }
        }
    };
    ($callback:expr, false) => {
        match Runtime::new() {
            Ok(v) => v,
            Err(_) => {
                call_callback_struct!("Error initializing runtime", $callback);
                return;
            }
        }
    }
}

#[macro_export]
macro_rules! call_callback_struct {
    ($string:expr, $callback:expr) => {
        let result = LicenseData::error($string);
        $callback(box_out!(result))
    };
}

/// With `true` argument set:
/// 
/// Parses a c_char and calls the callback with the given error message if 
/// there is an error.
/// 
/// With `false` argument set:
/// 
/// Parses a c_char and calls the callback with a nullptr with the given error message if there is an error.
#[macro_export]
macro_rules! parse_c_char {
    ($c_char_arg:expr, $error_message:expr, true) => {
        match unsafe { CStr::from_ptr($c_char_arg) }.to_str() {
            Ok(v) => v,
            Err(_) => {
                return box_out!(LicenseData::error($error_message))
            }
        }
    };
    ($c_char_arg:expr, $error_message:expr, $callback:expr, false) => {
        match unsafe { CStr::from_ptr($c_char_arg) }.to_str() {
            Ok(v) => v,
            Err(_) => {
                call_callback_struct!($error_message, $callback);
                return;
            }
        }
    }
}

/// Boxes a value that is being returned to the external code
#[macro_export]
macro_rules! box_out {
    ($data:expr) => {
        Box::into_raw(Box::new($data))
    };
}