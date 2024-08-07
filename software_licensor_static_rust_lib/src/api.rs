use std::{collections::HashMap, time::{SystemTime, UNIX_EPOCH}};

use aes_gcm::{aead::{Aead, Nonce}, Aes256Gcm, KeyInit};
use base64::prelude::{BASE64_STANDARD_NO_PAD, Engine as _};
use chacha20poly1305::ChaCha20Poly1305;
use p384::{ecdh::EphemeralSecret, ecdsa::{signature::DigestVerifier, Signature, VerifyingKey}, PublicKey};
use prost::Message;
use rand::{rngs::OsRng, RngCore};
use reqwest::Client;
use sha2::{Digest, Sha384};

pub(crate) type EcdsaDigest = Sha384;

use crate::{error::{Error, OptionErrors}, file_io::save_license_file, generated::software_licensor_client::{decrypt_info::ClientEcdhPubkey, ClientSideDataStorage, CompactServerEcdhKey, CompactServerEcdsaKey, DecryptInfo, LicenseActivationRequest, LicenseActivationResponse, PubkeyRepo, Request, Response}, LICENSE_ACTIVATION_URL, PUBLIC_KEY_REPO_URL};

/// Gets the Software Licensor Public Keys.
pub(crate) async fn get_pubkeys(data_storage: &mut ClientSideDataStorage, get_ecdh_key: bool) -> Result<(), Error> {
    let client = Client::new();
    let keys = client
        .get(PUBLIC_KEY_REPO_URL)
        .send()
        .await?;
    let pubkey_repo = match PubkeyRepo::decode_length_delimited(keys.bytes().await?) {
        Ok(v) => v,
        Err(_) => return Err(Error::ApiError("Pubkey repo was not decodable".to_string()))
    };
    // the amount of ecdh keys is a multiple of 2, so we can use a bitwise and to select a random one
    if get_ecdh_key {
        let ecdh_key = &pubkey_repo.ecdh_keys[OsRng.next_u32() as usize & (pubkey_repo.ecdh_keys.len() - 1)];
        data_storage.next_server_ecdh_key = Some(CompactServerEcdhKey {
            ecdh_key_id: ecdh_key.ecdh_key_id.clone(),
            ecdh_public_key: ecdh_key.ecdh_public_key.clone(),
        });
    }

    let ecdsa_key = &pubkey_repo.ecdsa_key.expect("protobuf should be formatted correctly");
    data_storage.server_ecdsa_key = Some(CompactServerEcdsaKey {
        ecdsa_key_id: ecdsa_key.ecdsa_key_id.to_owned(),
        ecdsa_public_key: ecdsa_key.ecdsa_public_key.to_owned(),
        expiration: ecdsa_key.expiration,
    });
    Ok(())
}

/// Performs an activate_license request.
/// 
/// Errors can include cryptography errors, LicensingErrors or ApiErrors.
pub(crate) async fn activate_license_request(store_id: &str, company_name_str: &str, product_ids: &Vec<&String>, machine_id: &str, license_code: &str, license_file: &mut ClientSideDataStorage) -> Result<(), Error> {
    license_file.license_code = license_code.to_string();

    let mut product_id_hashmap: HashMap<String, ()> = HashMap::with_capacity(product_ids.len());
    product_ids.iter().for_each(|product_id| {
        product_id_hashmap.insert(product_id.to_string(), ());
    });

    match &license_file.license_activation_response {
        Some(v) => {
            v.key_files.keys().for_each(|product_id| {
                product_id_hashmap.insert(product_id.to_string(), ());
            });
            v.licensing_errors.keys().for_each(|product_id| {
                product_id_hashmap.insert(product_id.to_string(), ());
            });
        },
        None => ()
    }
    let all_product_ids = product_id_hashmap.keys().cloned().collect::<Vec<String>>();

    if all_product_ids.is_empty() {
        return Err(Error::LicensingError((2, "".into())))
    }

    let inner_payload = LicenseActivationRequest {
        license_code: license_code.to_string(),
        machine_id: machine_id.to_string(),
        hardware_stats: license_file.machine_stats.clone(),
        product_ids: all_product_ids,
    };
    let inner_payload_bytes = inner_payload.encode_length_delimited_to_vec();

    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    let symmetric_algorithm = if is_x86_feature_detected!("aes") {
        "aes-256-gcm"
    } else {
        "chacha20-poly1305"
    };
    // aarch64 does not have runtime detection for NEON, so building this 
    // library using RustCrypto's chacha20-poly1305 will not enable NEON; there 
    // would need to be a compiler flag to enable the NEON code
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86")))]
    let symmetric_algorithm = "aes-256-gcm";

    let ephemeral_key = EphemeralSecret::random(&mut OsRng);
    let next_ecdh_key = match license_file.next_server_ecdh_key.unwrap_or_err("The next ECDH key was missing in the license file") {
        Ok(v) => v,
        Err(_) => {
            get_pubkeys(license_file, true).await?;
            license_file.next_server_ecdh_key.unwrap_or_err("Error getting next ECDH key")?
        }
    };
    let server_ecdh_pubkey = PublicKey::from_sec1_bytes(&next_ecdh_key.ecdh_public_key)?;

    let shared_secret = ephemeral_key.diffie_hellman(&server_ecdh_pubkey);
    let mut salt = [0u8; 48];
    OsRng.fill_bytes(&mut salt);
    let kdf = shared_secret.extract::<Sha384>(Some(&salt));
    
    let mut symmetric_key = [0u8; 32];
    let info = b"Software Licensor Authentication v2";
    kdf.expand(info, &mut symmetric_key).expect("This key is small enough");
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);

    let data = match symmetric_algorithm {
        "aes-256-gcm" => {
            let cipher = Aes256Gcm::new(&symmetric_key.into());
            let mut ciphertext = cipher.encrypt(&nonce.into(), inner_payload_bytes.as_slice())?;
            ciphertext.splice(0..0, nonce);
            ciphertext
        },
        "chacha20-poly1305" => {
            let cipher = ChaCha20Poly1305::new(&symmetric_key.into());
            let mut ciphertext = cipher.encrypt(&nonce.into(), inner_payload_bytes.as_slice())?;
            ciphertext.splice(0..0, nonce);
            ciphertext
        },
        _ => unreachable!()
    };

    let decryption_info = DecryptInfo {
        server_ecdh_key_id: next_ecdh_key.ecdh_key_id.clone(),
        ecdh_info: info.to_vec(),
        ecdh_salt: salt.to_vec(),
        client_ecdh_pubkey: Some(
            ClientEcdhPubkey::Der(
                ephemeral_key.public_key().to_sec1_bytes().to_vec()
            )
        ),
    };

    let mut server_ecdsa_key = license_file.server_ecdsa_key.unwrap_or_err("The server's ECDSA key was missing in the license file")?;
    if server_ecdsa_key.expiration < SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() {
        get_pubkeys(license_file, false).await?;
        server_ecdsa_key = license_file.server_ecdsa_key.unwrap_or_err("The server ECDSA key was not set in the license file")?;
    }

    let encapsulating_payload = Request {
        symmetric_algorithm: symmetric_algorithm.to_string(),
        client_id: store_id.to_string(),
        data,
        decryption_info: Some(decryption_info),
        server_ecdsa_key_id: server_ecdsa_key.ecdsa_key_id.clone(),
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    };

    let response = Client::new()
        .post(LICENSE_ACTIVATION_URL)
        .header("X-Signature", "None")
        .body(encapsulating_payload.encode_length_delimited_to_vec())
        .send()
        .await?;

    let status = response.status().as_u16();

    if status != 200 {
        let resp_text = response.text().await?;
        match resp_text.parse::<u32>() {
            Ok(v) => {
                // there was a licensing error with the request. These come in the
                // form of powers of 2 
                return Err(Error::LicensingError((v, license_code.to_string())))
            },
            Err(_) => {
                // there was a general error with the request
                return Err(Error::ApiError(resp_text))
            }
        }
    }

    let sig = response.headers().get("X-Signature").unwrap_or_err("The X-Signature header was missing")?.as_bytes();
    let binary_sig = match BASE64_STANDARD_NO_PAD.decode(sig) {
        Ok(v) => v,
        Err(_) => return Err(Error::ApiError("The signature was not base64 decodable".to_string()))
    };

    let signature: Signature = match Signature::from_der(&binary_sig) {
        Ok(v) => v,
        Err(_) => return Err(Error::ApiError("The signature was invalid".to_string()))
    };

    let response_bytes = response.bytes().await?;

    let verifying_key = match VerifyingKey::from_sec1_bytes(&server_ecdsa_key.ecdsa_public_key) {
        Ok(v) => v,
        Err(_) => return Err(Error::ApiError("The verifying key could not be decoded".to_string()))
    };

    match verifying_key.verify_digest(EcdsaDigest::new_with_prefix(&response_bytes), &signature) {
        Ok(_) => (),
        Err(_) => return Err(Error::ApiError("The signature did not match in the server's response".into()))
    }

    let response_wrapper = Response::decode_length_delimited(response_bytes).expect("If there was an error with the request, it would have been sent as a number or as text; otherwise, it would have been sent in the response wrapper");

    let next_ecdh_key = &response_wrapper.next_ecdh_key.unwrap_or_err("The response's ECDH key was None")?;
    license_file.next_server_ecdh_key = Some(CompactServerEcdhKey {
        ecdh_key_id: next_ecdh_key.ecdh_key_id.clone(),
        ecdh_public_key: next_ecdh_key.ecdh_public_key.clone(),
    });

    let ciphertext = response_wrapper.data;
    let nonce = &ciphertext[..12];
    let decrypted = match symmetric_algorithm {
        "aes-256-gcm" => {
            let cipher = Aes256Gcm::new(&symmetric_key.into());
            let n = Nonce::<Aes256Gcm>::from_slice(nonce);
            cipher.decrypt(n, &ciphertext[12..])?
        },
        "chacha20-poly1305" => {
            let cipher = ChaCha20Poly1305::new(&symmetric_key.into());
            let n = Nonce::<ChaCha20Poly1305>::from_slice(nonce);
            cipher.decrypt(n, &ciphertext[12..])?
        },
        _ => unreachable!()
    };

    let license_response = match LicenseActivationResponse::decode_length_delimited(decrypted.as_slice()) {
        Ok(v) => v,
        Err(e) => return Err(Error::ApiError(e.to_string()))
    };

    // save the license response
    license_file.license_activation_response = Some(license_response);
    save_license_file(license_file, company_name_str)?;

    Ok(())
}