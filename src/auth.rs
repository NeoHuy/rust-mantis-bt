use rand::Rng;
use sha2::{Digest, Sha256};

pub fn generate_cookie_string() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    let mut s = String::with_capacity(64);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

pub fn hash_password(plain: &str) -> String {
    let mut rng = rand::thread_rng();
    let salt_bytes: [u8; 16] = rng.gen();
    let mut salt = String::with_capacity(32);
    for b in salt_bytes {
        salt.push_str(&format!("{:02x}", b));
    }

    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(plain.as_bytes());
    let hash = hasher.finalize();

    let mut hash_str = String::with_capacity(64);
    for b in hash {
        hash_str.push_str(&format!("{:02x}", b));
    }

    format!("{}${}", salt, hash_str)
}

pub fn verify_password(plain: &str, stored: &str) -> bool {
    if let Some((salt, expected_hash)) = stored.split_once('$') {
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(plain.as_bytes());
        let hash = hasher.finalize();

        let mut computed = String::with_capacity(64);
        for b in hash {
            computed.push_str(&format!("{:02x}", b));
        }
        computed == expected_hash
    } else {
        // Plaintext fallback or legacy comparison
        stored == plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hash_and_verify() {
        let hashed = hash_password("root");
        assert!(verify_password("root", &hashed));
        assert!(!verify_password("wrong", &hashed));
    }

    #[test]
    fn test_generate_cookie_string() {
        let cookie1 = generate_cookie_string();
        let cookie2 = generate_cookie_string();
        assert_eq!(cookie1.len(), 64);
        assert_ne!(cookie1, cookie2);
    }
}
