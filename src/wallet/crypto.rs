use anyhow::{anyhow, Result};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;
use sha3::{Digest, Keccak256};

pub struct LocalKey {
    /// Lowercase hex, no `0x` prefix.
    pub private_key_hex: String,
    /// EIP-55 checksummed `0x…` address.
    pub address: String,
}

/// Generate a fresh secp256k1 keypair entirely on-device (no network access).
pub fn generate_local_key() -> Result<LocalKey> {
    // A random 32-byte string is outside the curve order with negligible probability;
    // retry defensively rather than panic.
    for _ in 0..16 {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| anyhow!("Secure random number generator unavailable: {}", e))?;
        if let Ok(secret) = SecretKey::from_slice(&bytes) {
            return Ok(LocalKey {
                private_key_hex: hex::encode(bytes),
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
    let bytes = hex::decode(raw).map_err(|_| anyhow!("Private key is not valid hex"))?;
    let secret =
        SecretKey::from_slice(&bytes).map_err(|_| anyhow!("Private key is not a valid scalar"))?;
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
        assert_eq!(address_from_private_key_hex(&a.private_key_hex)?, a.address);
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
}
