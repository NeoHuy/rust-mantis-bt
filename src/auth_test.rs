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
    assert!(hashed.starts_with("pbkdf2_sha256$"));
    let parts: Vec<&str> = hashed.split('$').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "pbkdf2_sha256");
    let iters: u32 = parts[1].parse().unwrap();
    assert!(iters >= pbkdf2_iterations());
    assert_eq!(parts[2].len(), 32); // 16 bytes salt as hex
    assert_eq!(parts[3].len(), 64); // pbkdf2-hmac-sha256 32 bytes as hex
    assert!(verify_password("secret_password", &hashed));
    assert!(!verify_password("wrong_password", &hashed));
    assert!(!needs_rehash(&hashed));

    // Legacy sha256$ format still verifies and triggers needs_rehash
    let legacy_salt = "0123456789abcdef0123456789abcdef";
    let mut legacy_hasher = Sha256::new();
    legacy_hasher.update(legacy_salt.as_bytes());
    legacy_hasher.update(b"secret_password");
    let legacy_hash = format!(
        "sha256${}${}",
        legacy_salt,
        hex_encode(&legacy_hasher.finalize())
    );
    assert!(verify_password("secret_password", &legacy_hash));
    assert!(needs_rehash(&legacy_hash));

    // Plaintext fallback is disabled by default
    assert!(!verify_password("plain_text", "plain_text"));
    assert!(!verify_password("", ""));
}

#[test]
fn test_pbkdf2_hmac_sha256_rfc6070_vectors() {
    let d1 = pbkdf2_hmac_sha256(b"password", b"salt", 1);
    assert_eq!(
        hex_encode(&d1),
        "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
    );

    let d2 = pbkdf2_hmac_sha256(b"password", b"salt", 2);
    assert_eq!(
        hex_encode(&d2),
        "ae4d0c95af6b46d32d0adff928f06dd02a303f8ef3c251dfd6e2d85a95474c43"
    );

    let d4096 = pbkdf2_hmac_sha256(b"password", b"salt", 4096);
    assert_eq!(
        hex_encode(&d4096),
        "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
    );
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
    assert!(db
        .get_user_by_cookie(&logged_in.cookie_string)
        .unwrap()
        .is_some());
}

#[test]
fn test_protected_and_last_admin_guards() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    // Cannot delete or demote the only administrator
    assert!(db.delete_user(admin.id).is_err());
    assert!(db
        .update_user(admin.id, "Admin", "a@b.c", None, 25, true, false)
        .is_err());

    // Create a protected reporter
    let uid = db
        .create_user("prot_user", "Prot", "p@b.c", "secret", 25, true, true)
        .unwrap();
    // Protected user cannot have password or access_level changed while protected=true
    assert!(db
        .update_user(uid, "Prot", "p@b.c", Some("newpass"), 55, true, true)
        .is_err());
    assert!(db.delete_user(uid).is_err());
}

#[test]
fn test_auth_disabled_user_login_denied_and_cookie_none() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();

    // Cannot disable the only administrator
    assert!(db
        .update_user(admin.id, "Admin", "a@b.c", None, 90, false, false)
        .is_err());

    // Create a disabled user
    let uid = db
        .create_user(
            "disabled_user",
            "Disabled",
            "d@b.c",
            "pass",
            25,
            false,
            false,
        )
        .unwrap();
    let user = db.get_user_by_id(uid).unwrap().unwrap();
    assert_eq!(user.enabled, 0);

    // Disabled user cannot log in
    assert!(db.authenticate("disabled_user", "pass").unwrap().is_none());
    let user_after = db.get_user_by_id(uid).unwrap().unwrap();
    assert_eq!(user_after.login_count, 0);

    // Disabled user cookie lookup returns None
    assert!(db
        .get_user_by_cookie(&user.cookie_string)
        .unwrap()
        .is_none());
}

#[test]
fn test_auth_user_crud_and_listing() {
    let db = Db::open_in_memory().unwrap();

    // Create user
    let dev_id = db
        .create_user(
            "developer1",
            "Dev One",
            "dev1@example.com",
            "pass123",
            55,
            true,
            false,
        )
        .unwrap();
    assert!(dev_id > 0);

    // Duplicate username should fail
    assert!(db
        .create_user(
            "developer1",
            "Dev Two",
            "dev2@example.com",
            "pass456",
            55,
            true,
            false
        )
        .is_err());

    // List users
    let all = db.list_users(None, false).unwrap();
    assert!(all.len() >= 2);

    let search_dev = db.list_users(Some("One"), false).unwrap();
    assert_eq!(search_dev.len(), 1);
    assert_eq!(search_dev[0].username, "developer1");

    // Non-protected user update password and access level
    assert!(db
        .update_user(
            dev_id,
            "Dev Updated",
            "dev_new@example.com",
            Some("newpass"),
            70,
            true,
            false
        )
        .is_ok());
    let updated = db.get_user_by_id(dev_id).unwrap().unwrap();
    assert_eq!(updated.realname, "Dev Updated");
    assert_eq!(updated.email, "dev_new@example.com");
    assert_eq!(updated.access_level, 70);
    assert!(verify_password("newpass", &updated.password));

    // Create another administrator
    let admin2_id = db
        .create_user(
            "admin2",
            "Admin Two",
            "admin2@example.com",
            "root2",
            90,
            true,
            false,
        )
        .unwrap();

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
    assert!(db
        .create_user("   ", "Real", "e@x.com", "pass", 25, true, false)
        .is_err());

    // Update/delete nonexistent user fails
    assert!(db
        .update_user(9999, "Real", "e@x.com", None, 25, true, false)
        .is_err());
    assert!(db.delete_user(9999).is_err());

    // Protected user can update realname and email when password is None and access level is unchanged
    let prot_id = db
        .create_user(
            "protected_dev",
            "Prot Dev",
            "p@x.com",
            "pass",
            55,
            true,
            true,
        )
        .unwrap();
    assert!(db
        .update_user(prot_id, "Prot Dev New", "pnew@x.com", None, 55, true, true)
        .is_ok());
    let prot_user = db.get_user_by_id(prot_id).unwrap().unwrap();
    assert_eq!(prot_user.realname, "Prot Dev New");
    assert_eq!(prot_user.email, "pnew@x.com");

    // Malformed stored passwords
    assert!(!verify_password(
        "pass",
        "sha256$malformed_no_second_dollar"
    ));
    assert!(!verify_password("pass", "sha256$"));

    // list_users with search AND enabled_only
    let _disabled_dev = db
        .create_user(
            "dev_disabled",
            "Dev Inactive",
            "in@x.com",
            "pass",
            55,
            false,
            false,
        )
        .unwrap();
    let active_devs = db.list_users(Some("dev"), true).unwrap();
    for u in &active_devs {
        assert_eq!(u.enabled, 1);
        assert!(
            u.username.contains("dev") || u.realname.contains("dev") || u.email.contains("dev")
        );
    }
    assert!(!active_devs.iter().any(|u| u.username == "dev_disabled"));
}

#[test]
fn test_auth_legacy_md5_fallback() {
    assert_eq!(md5_hex("root"), "63a9f0ea7bb98050796b649e85481845");
    assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
    assert!(verify_password("root", "63a9f0ea7bb98050796b649e85481845"));
    assert!(verify_password("root", "63A9F0EA7BB98050796B649E85481845"));
    assert!(!verify_password(
        "wrong",
        "63a9f0ea7bb98050796b649e85481845"
    ));
}

#[test]
fn test_constant_time_equality() {
    assert!(constant_time_eq(b"hello", b"hello"));
    assert!(!constant_time_eq(b"hello", b"world"));
    assert!(!constant_time_eq(b"hello", b"hell"));

    assert!(constant_time_eq_ignore_ascii_case("ABCdef", "abcdef"));
    assert!(constant_time_eq_ignore_ascii_case("63A9F0", "63a9f0"));
    assert!(!constant_time_eq_ignore_ascii_case("ABCdef", "xyz123"));
    assert!(!constant_time_eq_ignore_ascii_case("short", "longer_str"));
}

#[test]
fn test_session_rotation_on_login_logout_and_password_change() {
    let db = Db::open_in_memory().unwrap();
    let initial = db.get_user_by_username("administrator").unwrap().unwrap();
    let initial_cookie = initial.cookie_string.clone();

    // 1. Login rotates cookie_string
    let logged_in = db.authenticate("administrator", "root").unwrap().unwrap();
    assert_ne!(logged_in.cookie_string, initial_cookie);
    assert!(db.get_user_by_cookie(&initial_cookie).unwrap().is_none());
    assert!(db
        .get_user_by_cookie(&logged_in.cookie_string)
        .unwrap()
        .is_some());

    // 2. rotate_user_cookie invalidates current cookie
    let rotated = db.rotate_user_cookie(logged_in.id).unwrap();
    assert_ne!(rotated, logged_in.cookie_string);
    assert!(db
        .get_user_by_cookie(&logged_in.cookie_string)
        .unwrap()
        .is_none());
    assert!(db.get_user_by_cookie(&rotated).unwrap().is_some());

    // 3. Changing password via update_user rotates cookie_string
    db.update_user(
        logged_in.id,
        "Administrator",
        "root@localhost",
        Some("new_secret"),
        90,
        true,
        false,
    )
    .unwrap();
    let after_pw = db.get_user_by_id(logged_in.id).unwrap().unwrap();
    assert_ne!(after_pw.cookie_string, rotated);
    assert!(db.get_user_by_cookie(&rotated).unwrap().is_none());
    assert!(db
        .get_user_by_cookie(&after_pw.cookie_string)
        .unwrap()
        .is_some());
}

#[test]
fn test_brute_force_account_lockout() {
    let db = Db::open_in_memory().unwrap();
    let uid = db
        .create_user(
            "victim",
            "Victim",
            "v@example.com",
            "correctpass",
            25,
            true,
            false,
        )
        .unwrap();

    // 4 failed attempts do not lock yet
    for _ in 0..4 {
        assert!(db.authenticate("victim", "wrong").unwrap().is_none());
    }

    // 5th failed attempt locks the account
    assert!(db.authenticate("victim", "wrong").unwrap().is_none());

    // Subsequent attempt even with the CORRECT password is rejected with Err
    let locked_res = db.authenticate("victim", "correctpass");
    assert!(locked_res.is_err());
    assert!(locked_res.unwrap_err().contains("locked"));

    // Admin resetting password clears the lockout
    db.update_user(
        uid,
        "Victim",
        "v@example.com",
        Some("resetpass"),
        25,
        true,
        false,
    )
    .unwrap();
    assert!(db.authenticate("victim", "resetpass").unwrap().is_some());
}

#[test]
fn test_is_session_expired_and_cookie_attributes() {
    let db = Db::open_in_memory().unwrap();
    let mut user = db.get_user_by_username("administrator").unwrap().unwrap();

    // User with last_visit <= 0 (never logged in or invalid timestamp) is considered expired
    user.last_visit = 0;
    assert!(is_session_expired(&user));
    user.last_visit = -1;
    assert!(is_session_expired(&user));

    // Recent visit is not expired
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    user.last_visit = now - 60;
    assert!(!is_session_expired(&user));

    // Old visit beyond TTL (> 7 days) is expired
    user.last_visit = now - (session_ttl_secs() + 10);
    assert!(is_session_expired(&user));

    // make_cookie_header_with_path respects Path, HttpOnly, SameSite, Max-Age
    let hdr = make_cookie_header_with_path(
        "MANTIS_FLASH_TOKEN",
        "abc",
        "/api_tokens_page",
        true,
        Some(60),
    );
    assert!(hdr.contains("MANTIS_FLASH_TOKEN=abc"));
    assert!(hdr.contains("Path=/api_tokens_page"));
    assert!(hdr.contains("SameSite=Lax"));
    assert!(hdr.contains("HttpOnly"));
    assert!(hdr.contains("Max-Age=60"));
}

#[test]
#[test]
fn test_authenticate_auto_rehashes_legacy_passwords() {
    let db = Db::open_in_memory().unwrap();
    let uid_md5 = db
        .create_user("md5user", "MD5", "md5@example.com", "temp", 25, true, false)
        .unwrap();
    let uid_sha = db
        .create_user("shauser", "SHA", "sha@example.com", "temp", 25, true, false)
        .unwrap();

    // Seed legacy MD5 and legacy salted SHA-256 directly in DB
    let md5_pw = md5_hex("legacy_pass");
    let salt = "abcdef0123456789abcdef0123456789";
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b"legacy_pass");
    let sha_pw = format!("sha256${}${}", salt, hex_encode(&hasher.finalize()));

    {
        let conn = db.conn();
        conn.execute(
            "UPDATE mantis_user_table SET password = ?1 WHERE id = ?2",
            rusqlite::params![md5_pw, uid_md5],
        )
        .unwrap();
        conn.execute(
            "UPDATE mantis_user_table SET password = ?1 WHERE id = ?2",
            rusqlite::params![sha_pw, uid_sha],
        )
        .unwrap();
    }

    // Logging in upgrades both passwords to pbkdf2_sha256$
    let u_md5 = db.authenticate("md5user", "legacy_pass").unwrap().unwrap();
    assert!(u_md5.password.starts_with("pbkdf2_sha256$"));
    assert!(!needs_rehash(&u_md5.password));
    let u_md5_db = db.get_user_by_id(uid_md5).unwrap().unwrap();
    assert!(u_md5_db.password.starts_with("pbkdf2_sha256$"));

    let u_sha = db.authenticate("shauser", "legacy_pass").unwrap().unwrap();
    assert!(u_sha.password.starts_with("pbkdf2_sha256$"));
    assert!(!needs_rehash(&u_sha.password));
    let u_sha_db = db.get_user_by_id(uid_sha).unwrap().unwrap();
    assert!(u_sha_db.password.starts_with("pbkdf2_sha256$"));
}

#[test]
fn test_csrf_token_generation_and_verification() {
    let cookie = generate_cookie_string();
    let token = csrf_token_for_session(&cookie);
    assert_eq!(token.len(), 64);
    assert!(verify_csrf_token(&cookie, &token));
    assert!(!verify_csrf_token(&cookie, "invalid_token"));
    assert!(!verify_csrf_token("", &token));
    assert!(!verify_csrf_token(&cookie, ""));
}

#[test]
fn test_lockout_does_not_permanently_rearm() {
    let db = Db::open_in_memory().unwrap();
    let uid = db
        .create_user(
            "victim2",
            "Victim2",
            "v2@example.com",
            "correctpass",
            25,
            true,
            false,
        )
        .unwrap();

    // 5 failures lock the account
    for _ in 0..5 {
        assert!(db.authenticate("victim2", "wrong").unwrap().is_none());
    }
    assert!(db.authenticate("victim2", "correctpass").is_err());

    // Fast-forward past lockout window: simulate expired lock
    db.set_config("login_locked_until", 0, uid, "1").unwrap();

    // Next wrong attempt should NOT immediately re-lock the account on attempt 1!
    assert!(db.authenticate("victim2", "wrong").unwrap().is_none());
    // Correct password succeeds because failure counter was reset!
    assert!(db.authenticate("victim2", "correctpass").unwrap().is_some());
}

#[test]
fn test_admin_unlock_user() {
    let db = Db::open_in_memory().unwrap();
    let uid = db
        .create_user(
            "prot_victim",
            "Protected Victim",
            "pv@example.com",
            "pass",
            90,
            true,
            true, // protected = true
        )
        .unwrap();

    for _ in 0..5 {
        assert!(db.authenticate("prot_victim", "wrong").unwrap().is_none());
    }
    assert!(db.authenticate("prot_victim", "pass").is_err());

    // Explicit unlock by admin works even for protected users
    db.unlock_user(uid).unwrap();
    assert!(db.authenticate("prot_victim", "pass").unwrap().is_some());
}

#[test]
fn test_signup_session_has_valid_expiry() {
    let db = Db::open_in_memory().unwrap();
    let user = db
        .signup_user("signup_test", "Signup Test", "st@example.com", "pass123")
        .unwrap();
    // Newly signed-up user has last_visit > 0 and is not expired
    assert!(user.last_visit > 0);
    assert!(!is_session_expired(&user));

    // Manipulate last_visit to 0 or very old timestamp to verify expiry
    let mut old_user = user;
    old_user.last_visit = 0;
    assert!(is_session_expired(&old_user));

    old_user.last_visit = 1000;
    assert!(is_session_expired(&old_user));
}
}
