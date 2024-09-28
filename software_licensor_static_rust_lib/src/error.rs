use std::time::SystemTimeError;

#[derive(Debug)]
pub enum LicensingError {
    Success(String),
    NoLicenseFound(String),
    MachineLimitReached(String),
    TrialEnded(String),
    LicenseNoLongerActive(String),
    IncorrectOfflineCode(String),
    OfflineCodesNotAllowed(String),
    InvalidLicenseCode(String),
    MachineDeactivated(String),
    InvalidLicenseType(String),
    UnknownError((u32, String)),
}

impl From<LicensingError> for Error {
    fn from(value: LicensingError) -> Self {
        Error::LicensingError(value)
    }
}

#[derive(Debug)]
pub enum Error {
    LicensingError(LicensingError), // a licensing error, along with the license code
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

macro_rules! impl_error_codes {
    ($(($variant:ident, $val:literal)), *) => {
        impl From<(u32, String)> for LicensingError {
            fn from((error_code, license_code): (u32, String)) -> LicensingError {
                match error_code {
                    $(
                        $val => {LicensingError::$variant(license_code).into()}
                    )*
                    _ => LicensingError::UnknownError((error_code, license_code)).into()
                }
            }
        }

        impl LicensingError {
            #[inline(always)]
            pub fn get_error_code(&self) -> u32 {
                match self {
                    $(
                        Self::$variant(_) => $val,
                    )*
                    Self::UnknownError((error_code, _license_code)) => {
                        *error_code
                    }
                }
            }

            #[inline(always)]
            pub fn get_license_code(&self) -> String {
                match self {
                    $(
                        Self::$variant(license_code) => license_code.to_string(),
                    )*
                    Self::UnknownError((_error_code, license_code)) => {
                        license_code.to_string()
                    }
                }
            }
        }
    };
}

impl_error_codes!(
    (Success, 1), 
    (NoLicenseFound, 2),
    (MachineLimitReached, 4),
    (TrialEnded, 8),
    (LicenseNoLongerActive, 16),
    (IncorrectOfflineCode, 32),
    (OfflineCodesNotAllowed, 64),
    (InvalidLicenseCode, 128),
    (MachineDeactivated, 256),
    (InvalidLicenseType, 512)
);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(s) => f.write_str(s),
            Self::LicensingError(v) => f.write_str(&v.get_error_code().to_string()),
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