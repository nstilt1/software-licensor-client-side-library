use std::time::SystemTimeError;

#[derive(Debug)]
pub enum Error {
    LicensingError((u32, String)), // a licensing error, along with the license code
    ApiError(String), // an API error
    /// An IO error. This is usually caused when the program does not have sufficient 
    /// privileges to write to the output file
    IoError,
    /// This error should not happen; it is mainly here to prevent undefined behavior
    /// from panics
    OptionError(String),
    /// Crypto errors can occur when the code has been tampered with
    CryptoError(String),
    /// This error might happen when the server is unreachable, but could occur for
    /// other reasons.
    ReqwestError(reqwest::Error),
    SystemTimeError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(s) => f.write_str(s),
            Self::LicensingError((v, _license_code)) => f.write_str(&v.to_string()),
            Self::CryptoError(s) => f.write_str(s),
            Self::OptionError(s) => f.write_str(s),
            Self::IoError => f.write_str("There was an IO error"),
            Self::ReqwestError(e) => f.write_str(&e.to_string()),
            Self::SystemTimeError => f.write_str("There was an error getting the current time"),
        }
    }
}

pub trait OptionErrors<T: Sized> {
    fn unwrap_or_err(&self, error_message: &str) -> Result<&T, Error>;
}

impl<T: Sized> OptionErrors<T> for Option<T> {
    fn unwrap_or_err(&self, error_message: &str) -> Result<&T, Error> {
        if let Some(v) = self {
            Ok(v)
        } else {
            Err(Error::OptionError(error_message.to_string()))
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::ReqwestError(value)
    }
}

impl From<SystemTimeError> for Error {
    fn from(_value: SystemTimeError) -> Self {
        Self::SystemTimeError
    }
}

macro_rules! impl_string_error {
    ($error_type:ty, $error_enum:ident) => {
        impl From<$error_type> for Error {
            fn from(value: $error_type) -> Self {
                Self::$error_enum(value.to_string())
            }
        }
    }
}

impl_string_error!(p384::elliptic_curve::Error, CryptoError);
impl_string_error!(aes_gcm::Error, CryptoError);

macro_rules! impl_io_error {
    ($error_type:ty) => {
        impl From<$error_type> for Error {
            fn from(_value: $error_type) -> Self {
                Self::IoError
            }
        }
    };
}

impl_io_error!(std::io::Error);
impl_io_error!(std::env::VarError);