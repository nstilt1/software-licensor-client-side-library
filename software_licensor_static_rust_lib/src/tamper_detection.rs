#[cfg(all(target_os = "windows", not(debug_assertions)))]
//#[cfg(target_os = "windows")]
pub mod cert_verify {
    use std::ffi::c_void;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::BOOL;
    use windows_sys::Win32::Security::Cryptography::{
        CertCloseStore, CertFindCertificateInStore, CertFreeCertificateContext,
        CertGetNameStringW, CryptMsgClose, CryptMsgGetParam, CryptQueryObject,
        CERT_CONTEXT, CERT_FIND_SUBJECT_CERT, CERT_INFO,
        CERT_NAME_ATTR_TYPE,
        CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
        CERT_QUERY_FORMAT_FLAG_BINARY,
        CERT_QUERY_OBJECT_FILE,
        CMSG_SIGNER_INFO, CMSG_SIGNER_INFO_PARAM,
        HCERTSTORE,
        PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
    };
    use windows_sys::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
        WTD_CHOICE_FILE, WTD_REVOCATION_CHECK_NONE, WTD_REVOKE_NONE,
        WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };

    // HCRYPTMSG is not a named type in windows-sys 0.59 — it's *mut c_void
    type HCRYPTMSG = *mut c_void;

    const OID_COMMON_NAME: &[u8] = b"2.5.4.3\0";
    const OID_ORGANIZATION: &[u8] = b"2.5.4.10\0";

    // WINTRUST_ACTION_GENERIC_VERIFY_V2
    const WINTRUST_ACTION_GUID: windows_sys::core::GUID = windows_sys::core::GUID {
        data1: 0x00AAC56B,
        data2: 0xCD44,
        data3: 0x11d0,
        data4: [0x8C, 0xC2, 0x00, 0xC0, 0x4F, 0xC2, 0x95, 0xEE],
    };

    #[inline(always)]
    fn to_wide_null(s: &str) -> Vec<u16> {
        OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0u16))
            .collect()
    }

    /// Verifies the Authenticode trust chain via WinVerifyTrust.
    #[inline(always)]
    pub fn verify_authenticode(path: &str) -> Result<(), i32> {
        let wide_path = to_wide_null(path);

        let mut file_info = WINTRUST_FILE_INFO {
            cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
            pcwszFilePath: wide_path.as_ptr(),
            hFile: core::ptr::null_mut(),
            pgKnownSubject: std::ptr::null_mut(),
        };

        let mut trust_data = WINTRUST_DATA {
            cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
            pPolicyCallbackData: std::ptr::null_mut(),
            pSIPClientData: std::ptr::null_mut(),
            dwUIChoice: WTD_UI_NONE,
            fdwRevocationChecks: WTD_REVOKE_NONE,
            dwUnionChoice: WTD_CHOICE_FILE,
            Anonymous: WINTRUST_DATA_0 {
                pFile: &mut file_info,
            },
            dwStateAction: WTD_STATEACTION_VERIFY,
            hWVTStateData: core::ptr::null_mut(),
            pwszURLReference: std::ptr::null_mut(),
            dwProvFlags: WTD_REVOCATION_CHECK_NONE,
            dwUIContext: 0,
            pSignatureSettings: std::ptr::null_mut(),
        };

        let mut action_guid = WINTRUST_ACTION_GUID;

        let result = unsafe {
            WinVerifyTrust(
                -1isize as _, // INVALID_HANDLE_VALUE — no UI parent window
                &mut action_guid,
                &mut trust_data as *mut _ as *mut c_void,
            )
        };

        // Always release WinTrust state regardless of result
        trust_data.dwStateAction = WTD_STATEACTION_CLOSE;
        unsafe {
            WinVerifyTrust(
                -1isize as _,
                &mut action_guid,
                &mut trust_data as *mut _ as *mut c_void,
            );
        }

        if result == 0 {
            Ok(())
        } else {
            Err(result)
        }
    }

    /// RAII guard that closes the HCERTSTORE and HCRYPTMSG on drop.
    struct CryptoHandles {
        h_store: HCERTSTORE,
        h_msg: HCRYPTMSG,
    }

    impl Drop for CryptoHandles {
        fn drop(&mut self) {
            unsafe {
                if !self.h_msg.is_null() {
                    CryptMsgClose(self.h_msg);
                }
                if !self.h_store.is_null() {
                    CertCloseStore(self.h_store, 0);
                }
            }
        }
    }

    /// RAII guard that frees a CERT_CONTEXT on drop.
    struct CertContextGuard(*const CERT_CONTEXT);

    impl Drop for CertContextGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CertFreeCertificateContext(self.0) };
            }
        }
    }

    /// Opens the embedded PKCS#7 signature from a PE file and verifies
    /// that the signer's CN or O equals EXPECTED_NAME.
    #[inline(always)]
    pub fn verify_signer_identity(path: &str, expected_cn: &str, expected_o: &str) -> Result<(), String> {
        let wide_path = to_wide_null(path);

        let mut encoding_type: u32 = 0;
        let mut content_type: u32 = 0;
        let mut format_type: u32 = 0;
        // HCERTSTORE is *mut c_void in windows-sys 0.59
        let mut h_store: HCERTSTORE = std::ptr::null_mut();
        // HCRYPTMSG is also *mut c_void — not a named type in windows-sys 0.59
        let mut h_msg: HCRYPTMSG = std::ptr::null_mut();

        let ok: BOOL = unsafe {
            CryptQueryObject(
                CERT_QUERY_OBJECT_FILE,
                wide_path.as_ptr() as *const c_void,
                CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
                CERT_QUERY_FORMAT_FLAG_BINARY,
                0,
                &mut encoding_type,
                &mut content_type,
                &mut format_type,
                &mut h_store,
                // phmsg is *mut *mut c_void in windows-sys 0.59
                &mut h_msg as *mut HCRYPTMSG,
                std::ptr::null_mut(),
            )
        };

        if ok == 0 {
            return Err(format!(
                "CryptQueryObject failed: {:#010x}",
                unsafe { windows_sys::Win32::Foundation::GetLastError() }
            ));
        }

        let _handles = CryptoHandles { h_store, h_msg };

        // ── Step 1: get signer info size ──────────────────────────────────
        let mut signer_info_size: u32 = 0;
        let ok: BOOL = unsafe {
            CryptMsgGetParam(
                h_msg,
                CMSG_SIGNER_INFO_PARAM,
                0,
                std::ptr::null_mut(),
                &mut signer_info_size,
            )
        };
        if ok == 0 || signer_info_size == 0 {
            return Err("CryptMsgGetParam (size query) failed".to_string());
        }

        // ── Step 2: retrieve signer info ─────────────────────────────────
        let mut signer_info_buf = vec![0u8; signer_info_size as usize];
        let ok: BOOL = unsafe {
            CryptMsgGetParam(
                h_msg,
                CMSG_SIGNER_INFO_PARAM,
                0,
                signer_info_buf.as_mut_ptr() as *mut c_void,
                &mut signer_info_size,
            )
        };
        if ok == 0 {
            return Err("CryptMsgGetParam (data) failed".to_string());
        }

        let signer_info =
            unsafe { &*(signer_info_buf.as_ptr() as *const CMSG_SIGNER_INFO) };

        // ── Step 3: locate the signer's leaf certificate ──────────────────
        // Build a minimal CERT_INFO with just Issuer + SerialNumber for lookup
        let cert_info = CERT_INFO {
            dwVersion: 0,
            SerialNumber: signer_info.SerialNumber,
            SignatureAlgorithm: unsafe { std::mem::zeroed() },
            Issuer: signer_info.Issuer,
            NotBefore: unsafe { std::mem::zeroed() },
            NotAfter: unsafe { std::mem::zeroed() },
            Subject: unsafe { std::mem::zeroed() },
            SubjectPublicKeyInfo: unsafe { std::mem::zeroed() },
            IssuerUniqueId: unsafe { std::mem::zeroed() },
            SubjectUniqueId: unsafe { std::mem::zeroed() },
            cExtension: 0,
            rgExtension: std::ptr::null_mut(),
        };

        let p_cert = unsafe {
            CertFindCertificateInStore(
                h_store,
                X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                0,
                CERT_FIND_SUBJECT_CERT,
                &cert_info as *const CERT_INFO as *const c_void,
                std::ptr::null(),
            )
        };

        if p_cert.is_null() {
            return Err("Signer certificate not found in embedded store".to_string());
        }

        let _cert_guard = CertContextGuard(p_cert);

        // ── Step 4: extract CN and O, compare to expected ─────────────────
        let cn = get_name_attr(p_cert, OID_COMMON_NAME)
            .unwrap_or_default();
        let org = get_name_attr(p_cert, OID_ORGANIZATION)
            .unwrap_or_default();

        if cn != expected_cn && org != expected_o {
            return Err(format!(
                "Certificate mismatch — CN='{}', O='{}'; expected CN='{}' O='{}'",
                cn, org, expected_cn, expected_o
            ));
        }

        Ok(())
    }

    /// Retrieves a single name attribute (by dotted OID) from the cert subject.
    #[inline(always)]
    fn get_name_attr(p_cert: *const CERT_CONTEXT, oid: &[u8]) -> Option<String> {
        // First call: get required buffer length
        let len = unsafe {
            CertGetNameStringW(
                p_cert,
                CERT_NAME_ATTR_TYPE,
                0,
                oid.as_ptr() as *mut c_void,
                std::ptr::null_mut(),
                0,
            )
        };

        if len <= 1 {
            // len == 1 means only the NUL terminator — attribute absent
            return None;
        }

        let mut buf = vec![0u16; len as usize];
        unsafe {
            CertGetNameStringW(
                p_cert,
                CERT_NAME_ATTR_TYPE,
                0,
                oid.as_ptr() as *mut c_void,
                buf.as_mut_ptr(),
                len,
            )
        };

        // Strip NUL terminator and convert
        if let Some(nul) = buf.iter().position(|&c| c == 0) {
            buf.truncate(nul);
        }
        Some(String::from_utf16_lossy(&buf))
    }

    /// Runs both checks against the running executable.
    /// Call this near the top of `main()`.
    #[inline(always)]
    pub fn verify_self(expected_cn: &str, expected_o: &str) -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Cannot get exe path: {e}"))?;
        let exe_str = exe
            .to_str()
            .ok_or("Exe path is not valid UTF-8")?;

        verify_authenticode(exe_str)
            .map_err(|hr| format!("Authenticode check failed: HRESULT {hr:#010x}"))?;

        verify_signer_identity(exe_str, expected_cn, expected_o)?;

        Ok(())
    }
}

//#[cfg(all(target_os = "macos", not(debug_assertions)))]
#[cfg(target_os = "macos")]
pub mod cert_verify_mac {
    use core_foundation::{
        base::{CFOptionFlags, TCFType},
        string::{CFString, CFStringRef},
        url::{CFURL, CFURLRef},
    };
    use core_foundation::base::OSStatus;
    use security_framework_sys::base::errSecSuccess;
    use std::{path::Path, ptr};

    #[derive(Debug)]
    pub struct RequirementSpec<'a> {
        pub common_name: &'a str,
        pub organization: &'a str,
        pub team_id: Option<&'a str>, // optional but recommended
    }

    fn build_requirement(spec: &RequirementSpec) -> String {
        let mut req = format!(
            "anchor apple generic \
             and certificate leaf[subject.CN] = \"{}\" \
             and certificate leaf[subject.O] = \"{}\"",
            spec.common_name, spec.organization
        );

        if let Some(team) = spec.team_id {
            req.push_str(&format!(" and certificate leaf[subject.OU] = \"{}\"", team));
        }

        req
    }

    // SecStaticCode / SecRequirement are not fully exposed by the high-level
    // security-framework crate, so we bind the raw sys APIs directly.
    #[allow(non_camel_case_types)]
    type SecStaticCodeRef = *mut std::ffi::c_void;
    #[allow(non_camel_case_types)]
    type SecRequirementRef = *mut std::ffi::c_void;

    // kSecCSDefaultFlags = 0
    const SEC_CS_DEFAULT_FLAGS: CFOptionFlags = 0;
    // kSecCSCheckAllArchitectures = 1 — validates all slices in a universal binary
    const SEC_CS_CHECK_ALL_ARCHITECTURES: CFOptionFlags = 1;

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecStaticCodeCreateWithPath(
            path: CFURLRef,
            flags: CFOptionFlags,
            static_code: *mut SecStaticCodeRef,
        ) -> OSStatus;

        fn SecStaticCodeCheckValidity(
            static_code: SecStaticCodeRef,
            flags: CFOptionFlags,
            requirement: SecRequirementRef,
        ) -> OSStatus;

        fn SecRequirementCreateWithString(
            requirement_string: CFStringRef,
            flags: CFOptionFlags,
            requirement: *mut SecRequirementRef,
        ) -> OSStatus;

        fn CFRelease(cf: *const std::ffi::c_void);
    }

    /// Verifies the code signature and signer identity of the given path.
    ///
    /// Uses `SecStaticCodeCheckValidity` with a designated requirement string,
    /// which validates:
    ///   1. The code signature is structurally valid
    ///   2. The signing certificate chains to Apple's CA
    ///   3. The leaf cert's CN and O match the expected company name
    #[inline(always)]
        pub fn verify_signature(path: &Path, spec: &RequirementSpec) -> Result<(), String> {
        let path_str = path.to_str().ok_or("Path is not valid UTF-8")?;

        let cf_path = CFString::new(path_str);
        let cf_url = CFURL::from_file_system_path(
            cf_path,
            core_foundation::url::kCFURLPOSIXPathStyle,
            false,
        );

        // Create SecStaticCode
        let mut static_code: SecStaticCodeRef = ptr::null_mut();
        let status = unsafe {
            SecStaticCodeCreateWithPath(
                cf_url.as_concrete_TypeRef(),
                0,
                &mut static_code,
            )
        };

        if status != errSecSuccess {
            return Err(format!("SecStaticCodeCreateWithPath failed: OSStatus {status}"));
        }

        struct CodeGuard(SecStaticCodeRef);
        impl Drop for CodeGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe { CFRelease(self.0) };
                }
            }
        }
        let _code_guard = CodeGuard(static_code);

        // Build requirement string
        let req_string = build_requirement(spec);
        let req_cf = CFString::new(&req_string);

        let mut requirement: SecRequirementRef = ptr::null_mut();
        let status = unsafe {
            SecRequirementCreateWithString(
                req_cf.as_concrete_TypeRef(),
                0,
                &mut requirement,
            )
        };

        if status != errSecSuccess {
            return Err(format!(
                "SecRequirementCreateWithString failed: OSStatus {status}"
            ));
        }

        struct ReqGuard(SecRequirementRef);
        impl Drop for ReqGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe { CFRelease(self.0) };
                }
            }
        }
        let _req_guard = ReqGuard(requirement);

        // Validate signature
        let status = unsafe {
            SecStaticCodeCheckValidity(
                static_code,
                1, // SEC_CS_CHECK_ALL_ARCHITECTURES
                requirement,
            )
        };

        match status {
            s if s == errSecSuccess => Ok(()),
            -67050 => Err("Binary is not code-signed".to_string()),
            -67030 => Err(format!("Code signature requirement not satisfied: {req_string}")),
            -67062 => Err("Code signature is invalid/corrupted".to_string()),
            s => Err(format!("SecStaticCodeCheckValidity failed: OSStatus {s}")),
        }
    }

    /// Verifies the running executable itself.
    /// Call near the top of `main()`.
    #[inline(always)]
    pub fn verify_self(
        common_name: &str,
        organization: &str,
        team_id: Option<&str>,
    ) -> Result<(), String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Cannot get exe path: {e}"))?;

        let spec = RequirementSpec {
            common_name,
            organization,
            team_id,
        };

        verify_signature(&exe, &spec)
    }
}

/// Verifies the certificate and its metadata.
/// 
/// # MacOS:
/// CN = `Developer ID Application: First Last (WK12345678)`
/// O = `First Last`
/// OU/Team ID = WK12345678
/// 
/// # Windows:
/// CN = CN
/// O = O
/// Team ID = None
pub fn verify_sig(common_name: &str, organization: &str, team_id: Option<&str>) -> Result<(), String> {
    #[cfg(all(target_os = "macos", not(debug_assertions)))]
    return cert_verify_mac::verify_self(common_name, organization, team_id);
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    return cert_verify::verify_self(common_name, organization);
    #[cfg(
        any(
            not(any(target_os = "macos", target_os = "windows")),
            debug_assertions
        )
    )]
    Ok(())
}