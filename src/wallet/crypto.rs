use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;
use ring::{aead, hkdf, pbkdf2};
use sha3::{Digest, Keccak256};
use std::num::NonZeroU32;
use zeroize::Zeroizing;

pub const ENCRYPTED_PRIVKEY_PREFIX: &str = "enc:v1:aes256gcm:";
const PRIVKEY_HKDF_INFO: &[u8] = b"cae-cli:wallet-privkey:v1";
const PBKDF2_ITERATIONS: u32 = 100_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Clone, Copy)]
pub enum WrappingKey<'a> {
    Passphrase(&'a str),
    MasterKey(&'a [u8; 32]),
}

pub enum KeyMaterial {
    Passphrase(Zeroizing<String>),
    MasterKey(Zeroizing<[u8; 32]>),
}

pub struct LocalKey {
    /// Lowercase hex, no `0x` prefix. Automatically zeroized on drop.
    pub private_key_hex: Zeroizing<String>,
    /// EIP-55 checksummed `0x…` address.
    pub address: String,
}

/// Generate a fresh secp256k1 keypair entirely on-device (no network access).
pub fn generate_local_key() -> Result<LocalKey> {
    // A random 32-byte string is outside the curve order with negligible probability;
    // retry defensively rather than panic.
    for _ in 0..16 {
        let mut bytes = Zeroizing::new([0u8; 32]);
        getrandom::getrandom(bytes.as_mut_slice())
            .map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;
        if let Ok(secret) = SecretKey::from_slice(bytes.as_slice()) {
            return Ok(LocalKey {
                private_key_hex: Zeroizing::new(hex::encode(bytes.as_slice())),
                address: address_from_secret(&secret),
            });
        }
    }
    Err(anyhow!("Failed to generate a valid secp256k1 private key"))
}

/// Derive the EIP-55 checksummed address for a secret key.
pub fn address_from_secret(secret: &SecretKey) -> String {
    let point = secret.public_key().to_encoded_point(false);
    // Uncompressed SEC1 point is 0x04 || X || Y; the address hashes X || Y.
    let digest = Keccak256::digest(&point.as_bytes()[1..]);
    to_checksum_address(&digest[12..])
}

/// Derive the checksummed address from a hex private key (with or without `0x`).
pub fn address_from_private_key_hex(private_key_hex: &str) -> Result<String> {
    let raw = private_key_hex.trim_start_matches("0x");
    let bytes =
        Zeroizing::new(hex::decode(raw).map_err(|_| anyhow!("Private key is not valid hex"))?);
    let secret = SecretKey::from_slice(bytes.as_slice())
        .map_err(|_| anyhow!("Private key is not a valid scalar"))?;
    Ok(address_from_secret(&secret))
}

/// EIP-55 mixed-case checksum encoding of a 20-byte address.
pub fn to_checksum_address(address: &[u8]) -> String {
    let lower = hex::encode(address);
    let hash = hex::encode(Keccak256::digest(lower.as_bytes()));
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (c, h) in lower.chars().zip(hash.chars()) {
        if c.is_ascii_alphabetic() && h >= '8' {
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// True for a 42-character `0x` + 40 hex digit string.
pub fn is_valid_address(address: &str) -> bool {
    address.len() == 42
        && address.starts_with("0x")
        && address[2..].chars().all(|c| c.is_ascii_hexdigit())
}

pub fn is_encrypted_privkey(value: &str) -> bool {
    value.starts_with(ENCRYPTED_PRIVKEY_PREFIX)
}

fn derive_aes_key(wrapping: WrappingKey<'_>, salt: &[u8; SALT_LEN]) -> Result<Zeroizing<[u8; 32]>> {
    let mut out_key = Zeroizing::new([0u8; 32]);
    match wrapping {
        WrappingKey::Passphrase(pass) => {
            if pass.is_empty() {
                bail!("Wallet passphrase must not be empty");
            }
            let iterations = NonZeroU32::new(PBKDF2_ITERATIONS).expect("non-zero PBKDF2 rounds");
            pbkdf2::derive(
                pbkdf2::PBKDF2_HMAC_SHA256,
                iterations,
                salt,
                pass.as_bytes(),
                out_key.as_mut_slice(),
            );
        }
        WrappingKey::MasterKey(master) => {
            let hkdf_salt = hkdf::Salt::new(hkdf::HKDF_SHA256, salt);
            let prk = hkdf_salt.extract(master);
            let okm = prk
                .expand(&[PRIVKEY_HKDF_INFO], &aead::AES_256_GCM)
                .map_err(|_| anyhow!("HKDF key expansion failed"))?;
            okm.fill(out_key.as_mut_slice())
                .map_err(|_| anyhow!("HKDF key fill failed"))?;
        }
    }
    Ok(out_key)
}

/// Encrypt a 32-byte secp256k1 private key hex string into a versioned `enc:v1:aes256gcm:...` envelope.
pub fn encrypt_private_key_hex(
    private_key_hex: &str,
    evm_address: &str,
    wrapping: WrappingKey<'_>,
) -> Result<String> {
    let raw = private_key_hex.trim().trim_start_matches("0x");
    let raw_bytes =
        Zeroizing::new(hex::decode(raw).map_err(|_| anyhow!("Private key is not valid hex"))?);
    if raw_bytes.len() != 32 || SecretKey::from_slice(raw_bytes.as_slice()).is_err() {
        bail!("Private key must be a valid 32-byte secp256k1 scalar");
    }

    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt)
        .map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes)
        .map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;

    let aes_key = derive_aes_key(wrapping, &salt)?;
    let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, aes_key.as_slice())
        .map_err(|_| anyhow!("Invalid AES-256-GCM key"))?;
    let key = aead::LessSafeKey::new(unbound);

    let aad_addr = evm_address.to_ascii_lowercase();
    let aad = aead::Aad::from(aad_addr.as_bytes());
    let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
    let mut buf = Vec::with_capacity(raw_bytes.len() + aead::AES_256_GCM.tag_len());
    buf.extend_from_slice(raw_bytes.as_slice());
    let mut in_out = Zeroizing::new(buf);
    key.seal_in_place_append_tag(nonce, aad, &mut *in_out)
        .map_err(|_| anyhow!("Failed to encrypt private key"))?;

    let kdf = match wrapping {
        WrappingKey::Passphrase(_) => "pbkdf2",
        WrappingKey::MasterKey(_) => "hkdf",
    };

    Ok(format!(
        "{}{}:{}:{}:{}",
        ENCRYPTED_PRIVKEY_PREFIX,
        kdf,
        URL_SAFE_NO_PAD.encode(salt),
        URL_SAFE_NO_PAD.encode(nonce_bytes),
        URL_SAFE_NO_PAD.encode(in_out.as_slice()),
    ))
}

/// Decrypt an `enc:v1:aes256gcm:...` envelope into a zeroizing 64-char lowercase private key hex string.
pub fn decrypt_private_key_hex_with(
    envelope: &str,
    evm_address: &str,
    resolver: impl FnOnce(&str) -> Result<KeyMaterial>,
) -> Result<Zeroizing<String>> {
    let body = envelope
        .strip_prefix(ENCRYPTED_PRIVKEY_PREFIX)
        .ok_or_else(|| anyhow!("Unsupported encrypted private key envelope format"))?;
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() != 4 {
        bail!("Malformed encrypted private key envelope");
    }
    let kdf = parts[0];
    let salt_vec = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| anyhow!("Invalid salt encoding in encrypted private key"))?;
    let salt: [u8; SALT_LEN] = salt_vec
        .try_into()
        .map_err(|_| anyhow!("Invalid salt length in encrypted private key"))?;

    let nonce_vec = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| anyhow!("Invalid nonce encoding in encrypted private key"))?;
    let nonce_bytes: [u8; NONCE_LEN] = nonce_vec
        .try_into()
        .map_err(|_| anyhow!("Invalid nonce length in encrypted private key"))?;

    let mut in_out = Zeroizing::new(
        URL_SAFE_NO_PAD
            .decode(parts[3])
            .map_err(|_| anyhow!("Invalid ciphertext encoding in encrypted private key"))?,
    );

    let material = resolver(kdf)?;
    let aes_key = match (kdf, &material) {
        ("pbkdf2", KeyMaterial::Passphrase(pass)) => {
            derive_aes_key(WrappingKey::Passphrase(pass.as_str()), &salt)?
        }
        ("hkdf", KeyMaterial::MasterKey(master)) => {
            derive_aes_key(WrappingKey::MasterKey(master), &salt)?
        }
        _ => bail!(
            "Unsupported or mismatched KDF '{}' for private key decryption",
            kdf
        ),
    };

    let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, aes_key.as_slice())
        .map_err(|_| anyhow!("Invalid AES-256-GCM key"))?;
    let key = aead::LessSafeKey::new(unbound);

    let aad_addr = evm_address.to_ascii_lowercase();
    let aad = aead::Aad::from(aad_addr.as_bytes());
    let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);
    let plaintext = key.open_in_place(nonce, aad, &mut in_out).map_err(|_| {
        anyhow!(
            "Failed to decrypt stored private key (wrong passphrase, keyfile, or corrupted wallet)"
        )
    })?;
    if plaintext.len() != 32 || SecretKey::from_slice(plaintext).is_err() {
        bail!("Decrypted private key payload is invalid");
    }
    Ok(Zeroizing::new(hex::encode(plaintext)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_known_address_for_private_key_one() -> Result<()> {
        let key = format!("{:064x}", 1);
        let addr = address_from_private_key_hex(&key)?;
        assert_eq!(addr, "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf");
        Ok(())
    }

    #[test]
    fn matches_eip55_reference_vectors() -> Result<()> {
        for expected in [
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed",
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359",
            "0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB",
            "0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb",
        ] {
            let bytes = hex::decode(&expected[2..])?;
            assert_eq!(to_checksum_address(&bytes), expected);
        }
        Ok(())
    }

    #[test]
    fn generated_keys_are_valid_and_unique() -> Result<()> {
        let a = generate_local_key()?;
        let b = generate_local_key()?;
        assert!(is_valid_address(&a.address));
        assert_eq!(a.private_key_hex.len(), 64);
        assert_ne!(a.private_key_hex, b.private_key_hex);
        assert_eq!(
            address_from_private_key_hex(a.private_key_hex.as_str())?,
            a.address
        );
        Ok(())
    }

    #[test]
    fn local_key_private_hex_is_zeroizing_and_clears_on_zeroize() -> Result<()> {
        use zeroize::Zeroize;

        let mut key = generate_local_key()?;
        let hex_guard: &Zeroizing<String> = &key.private_key_hex;
        assert_eq!(hex_guard.len(), 64);

        key.private_key_hex.zeroize();
        assert!(key.private_key_hex.is_empty());
        assert!(is_valid_address(&key.address));

        let mut raw = Zeroizing::new([0xAAu8; 32]);
        raw.zeroize();
        assert_eq!(*raw, [0u8; 32]);

        let mut decoded = Zeroizing::new(vec![0xBBu8; 32]);
        decoded.zeroize();
        assert!(decoded.is_empty());
        Ok(())
    }

    #[test]
    fn rejects_bad_keys() {
        assert!(address_from_private_key_hex("nothex").is_err());
        assert!(address_from_private_key_hex(&"00".repeat(32)).is_err());
    }

    #[test]
    fn validates_address_shape() {
        assert!(is_valid_address(
            "0x7E5F4552091A69125d5DfCb7b8C2659029395Bdf"
        ));
        assert!(!is_valid_address("0x123"));
        assert!(!is_valid_address(&format!("0x{}", "z".repeat(40))));
    }

    #[test]
    fn encrypts_and_decrypts_private_key_with_hkdf_and_pbkdf2() -> Result<()> {
        let generated = generate_local_key()?;
        let master = [0x42u8; 32];

        let hkdf_env = encrypt_private_key_hex(
            generated.private_key_hex.as_str(),
            &generated.address,
            WrappingKey::MasterKey(&master),
        )?;
        assert!(is_encrypted_privkey(&hkdf_env));
        assert!(hkdf_env.starts_with("enc:v1:aes256gcm:hkdf:"));
        assert!(!hkdf_env.contains(generated.private_key_hex.as_str()));

        let decrypted_hkdf = decrypt_private_key_hex_with(&hkdf_env, &generated.address, |kdf| {
            assert_eq!(kdf, "hkdf");
            Ok(KeyMaterial::MasterKey(Zeroizing::new(master)))
        })?;
        assert_eq!(decrypted_hkdf.as_str(), generated.private_key_hex.as_str());

        let pbkdf2_env = encrypt_private_key_hex(
            generated.private_key_hex.as_str(),
            &generated.address,
            WrappingKey::Passphrase("correct horse battery staple"),
        )?;
        assert!(pbkdf2_env.starts_with("enc:v1:aes256gcm:pbkdf2:"));
        assert!(!pbkdf2_env.contains(generated.private_key_hex.as_str()));

        let decrypted_pbkdf2 =
            decrypt_private_key_hex_with(&pbkdf2_env, &generated.address, |kdf| {
                assert_eq!(kdf, "pbkdf2");
                Ok(KeyMaterial::Passphrase(Zeroizing::new(
                    "correct horse battery staple".to_string(),
                )))
            })?;
        assert_eq!(
            decrypted_pbkdf2.as_str(),
            generated.private_key_hex.as_str()
        );

        // Wrong passphrase must fail.
        assert!(
            decrypt_private_key_hex_with(&pbkdf2_env, &generated.address, |_| {
                Ok(KeyMaterial::Passphrase(Zeroizing::new(
                    "wrong passphrase".to_string(),
                )))
            })
            .is_err()
        );

        // Wrong AAD address must fail.
        assert!(decrypt_private_key_hex_with(
            &hkdf_env,
            "0x1111111111111111111111111111111111111111",
            |_| Ok(KeyMaterial::MasterKey(Zeroizing::new(master))),
        )
        .is_err());

        // Tampered ciphertext must fail.
        let mut tampered = hkdf_env.clone();
        let last = tampered.pop().unwrap_or('A');
        tampered.push(if last == 'A' { 'B' } else { 'A' });
        assert!(
            decrypt_private_key_hex_with(&tampered, &generated.address, |_| {
                Ok(KeyMaterial::MasterKey(Zeroizing::new(master)))
            })
            .is_err()
        );
        Ok(())
    }
}
