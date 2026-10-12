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

    format!("sha256${}${}", salt, hash_str)
}

pub fn verify_password(plain: &str, stored: &str) -> bool {
    if let Some(rest) = stored.strip_prefix("sha256$") {
        if let Some((salt, expected_hash)) = rest.split_once('$') {
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
            false
        }
    } else if let Some((salt, expected_hash)) = stored.split_once('$') {
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
    use crate::db::Db;

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

    #[test]
    fn test_auth_password_hashing_sha256_format_and_legacy_plain() {
        let hashed = hash_password("secret_password");
        assert!(hashed.starts_with("sha256$"));
        let parts: Vec<&str> = hashed.split('$').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "sha256");
        assert_eq!(parts[1].len(), 32); // 16 bytes salt as hex
        assert_eq!(parts[2].len(), 64); // sha256 output as hex
        assert!(verify_password("secret_password", &hashed));
        assert!(!verify_password("wrong_password", &hashed));

        // Legacy plain fallback
        assert!(verify_password("plain_text", "plain_text"));
        assert!(!verify_password("wrong_text", "plain_text"));
    }

    #[test]
    fn test_auth_login_counters_and_disabled_user() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.authenticate("administrator", "wrong").unwrap().is_none());
        let u = db.get_user_by_username("administrator").unwrap().unwrap();
        assert_eq!(u.failed_login_count, 1);

        let logged_in = db.authenticate("administrator", "root").unwrap().unwrap();
        assert_eq!(logged_in.failed_login_count, 0);
        assert_eq!(logged_in.login_count, 1);
        assert!(db.get_user_by_cookie(&logged_in.cookie_string).unwrap().is_some());
    }

    #[test]
    fn test_protected_and_last_admin_guards() {
        let db = Db::open_in_memory().unwrap();
        let admin = db.get_user_by_username("administrator").unwrap().unwrap();
        // Cannot delete or demote the only administrator
        assert!(db.delete_user(admin.id).is_err());
        assert!(db.update_user(admin.id, "Admin", "a@b.c", None, 25, true, false).is_err());

        // Create a protected reporter
        let uid = db.create_user("prot_user", "Prot", "p@b.c", "secret", 25, true, true).unwrap();
        // Protected user cannot have password or access_level changed while protected=true
        assert!(db.update_user(uid, "Prot", "p@b.c", Some("newpass"), 55, true, true).is_err());
        assert!(db.delete_user(uid).is_err());
    }

    #[test]
    fn test_auth_disabled_user_login_denied_and_cookie_none() {
        let db = Db::open_in_memory().unwrap();
        let admin = db.get_user_by_username("administrator").unwrap().unwrap();

        // Cannot disable the only administrator
        assert!(db.update_user(admin.id, "Admin", "a@b.c", None, 90, false, false).is_err());

        // Create a disabled user
        let uid = db.create_user("disabled_user", "Disabled", "d@b.c", "pass", 25, false, false).unwrap();
        let user = db.get_user_by_id(uid).unwrap().unwrap();
        assert_eq!(user.enabled, 0);

        // Disabled user cannot log in
        assert!(db.authenticate("disabled_user", "pass").unwrap().is_none());
        let user_after = db.get_user_by_id(uid).unwrap().unwrap();
        assert_eq!(user_after.login_count, 0);

        // Disabled user cookie lookup returns None
        assert!(db.get_user_by_cookie(&user.cookie_string).unwrap().is_none());
    }

    #[test]
    fn test_auth_user_crud_and_listing() {
        let db = Db::open_in_memory().unwrap();

        // Create user
        let dev_id = db.create_user("developer1", "Dev One", "dev1@example.com", "pass123", 55, true, false).unwrap();
        assert!(dev_id > 0);

        // Duplicate username should fail
        assert!(db.create_user("developer1", "Dev Two", "dev2@example.com", "pass456", 55, true, false).is_err());

        // List users
        let all = db.list_users(None, false).unwrap();
        assert!(all.len() >= 2);

        let search_dev = db.list_users(Some("One"), false).unwrap();
        assert_eq!(search_dev.len(), 1);
        assert_eq!(search_dev[0].username, "developer1");

        // Non-protected user update password and access level
        assert!(db.update_user(dev_id, "Dev Updated", "dev_new@example.com", Some("newpass"), 70, true, false).is_ok());
        let updated = db.get_user_by_id(dev_id).unwrap().unwrap();
        assert_eq!(updated.realname, "Dev Updated");
        assert_eq!(updated.email, "dev_new@example.com");
        assert_eq!(updated.access_level, 70);
        assert!(verify_password("newpass", &updated.password));

        // Create another administrator
        let admin2_id = db.create_user("admin2", "Admin Two", "admin2@example.com", "root2", 90, true, false).unwrap();

        // Now we can demote or delete the original admin because another admin exists
        let admin1 = db.get_user_by_username("administrator").unwrap().unwrap();
        assert!(db.delete_user(admin1.id).is_ok());

        // But now admin2 is the only admin, cannot delete admin2
        assert!(db.delete_user(admin2_id).is_err());
    }

    #[test]
    fn test_auth_edge_cases() {
        let db = Db::open_in_memory().unwrap();

        // Nonexistent user login returns Ok(None)
        assert!(db.authenticate("nonexistent", "secret").unwrap().is_none());

        // Empty username creation fails
        assert!(db.create_user("   ", "Real", "e@x.com", "pass", 25, true, false).is_err());

        // Update/delete nonexistent user fails
        assert!(db.update_user(9999, "Real", "e@x.com", None, 25, true, false).is_err());
        assert!(db.delete_user(9999).is_err());

        // Protected user can update realname and email when password is None and access level is unchanged
        let prot_id = db.create_user("protected_dev", "Prot Dev", "p@x.com", "pass", 55, true, true).unwrap();
        assert!(db.update_user(prot_id, "Prot Dev New", "pnew@x.com", None, 55, true, true).is_ok());
        let prot_user = db.get_user_by_id(prot_id).unwrap().unwrap();
        assert_eq!(prot_user.realname, "Prot Dev New");
        assert_eq!(prot_user.email, "pnew@x.com");

        // Malformed stored passwords
        assert!(!verify_password("pass", "sha256$malformed_no_second_dollar"));
        assert!(!verify_password("pass", "sha256$"));

        // list_users with search AND enabled_only
        let _disabled_dev = db.create_user("dev_disabled", "Dev Inactive", "in@x.com", "pass", 55, false, false).unwrap();
        let active_devs = db.list_users(Some("dev"), true).unwrap();
        for u in &active_devs {
            assert_eq!(u.enabled, 1);
            assert!(u.username.contains("dev") || u.realname.contains("dev") || u.email.contains("dev"));
        }
        assert!(!active_devs.iter().any(|u| u.username == "dev_disabled"));
    }
}
