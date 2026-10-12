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

fn verify_sha256_salted(plain: &str, salt: &str, expected_hash: &str) -> bool {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(plain.as_bytes());
    let hash = hasher.finalize();

    let mut computed = String::with_capacity(64);
    for b in hash {
        computed.push_str(&format!("{:02x}", b));
    }
    computed.eq_ignore_ascii_case(expected_hash)
}

pub fn md5_hex(input: &str) -> String {
    let input_bytes = input.as_bytes();
    let bit_len = (input_bytes.len() as u64).wrapping_mul(8);
    let mut msg = input_bytes.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    let s = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let k: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
        }

        let mut a = a0;
        let mut b = b0;
        let mut c = c0;
        let mut d = d0;

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };

            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(k[i])
                    .wrapping_add(m[g])
                    .rotate_left(s[i]),
            );
            a = temp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = String::with_capacity(32);
    for word in [a0, b0, c0, d0] {
        for byte in word.to_le_bytes() {
            out.push_str(&format!("{:02x}", byte));
        }
    }
    out
}

pub fn verify_password(plain: &str, stored: &str) -> bool {
    if let Some(rest) = stored.strip_prefix("sha256$") {
        if let Some((salt, expected_hash)) = rest.split_once('$') {
            return verify_sha256_salted(plain, salt, expected_hash);
        }
        return false;
    }

    if let Some((salt, expected_hash)) = stored.split_once('$') {
        return verify_sha256_salted(plain, salt, expected_hash);
    }

    if stored.len() == 32 && stored.chars().all(|c| c.is_ascii_hexdigit()) && md5_hex(plain).eq_ignore_ascii_case(stored) {
        return true;
    }

    // Plaintext fallback or legacy comparison
    stored == plain
}

pub fn effective_access_level(
    db: &crate::db::Db,
    user: &crate::models::User,
    project_id: i64,
) -> rusqlite::Result<i32> {
    db.effective_access_level(user, project_id)
}

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub user: crate::models::User,
    pub active_project_id: i64,
    pub visible_projects: Vec<(crate::models::Project, usize)>,
}

pub fn parse_cookies(headers: &axum::http::HeaderMap) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for header_val in headers.get_all(axum::http::header::COOKIE) {
        if let Ok(cookie_val) = header_val.to_str() {
            for piece in cookie_val.split(';') {
                let piece = piece.trim();
                if let Some((k, v)) = piece.split_once('=') {
                    map.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
    }
    map
}

pub fn make_cookie_header(name: &str, value: &str, http_only: bool, max_age: Option<i64>) -> String {
    let mut parts = vec![
        format!("{}={}", name, value),
        "Path=/".to_string(),
        "SameSite=Lax".to_string(),
    ];
    if http_only {
        parts.push("HttpOnly".to_string());
    }
    if let Some(age) = max_age {
        parts.push(format!("Max-Age={}", age));
    }
    parts.join("; ")
}

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AuthContext
where
    crate::db::Db: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        use axum::extract::FromRef;
        use axum::response::IntoResponse;

        let db = crate::db::Db::from_ref(state);
        let cookies = parse_cookies(&parts.headers);
        let cookie_string = match cookies.get("MANTIS_STRING_COOKIE") {
            Some(c) if !c.is_empty() => c,
            _ => {
                return Err(axum::response::Redirect::to("/login").into_response());
            }
        };

        let user = match db.get_user_by_cookie(cookie_string) {
            Ok(Some(u)) if u.enabled == 1 => u,
            _ => {
                return Err(axum::response::Redirect::to("/login").into_response());
            }
        };

        let mut active_project_id = 0;
        if let Some(pid_str) = cookies.get("MANTIS_PROJECT_COOKIE") {
            if let Ok(pid) = pid_str.parse::<i64>() {
                if pid > 0 {
                    if let Ok(level) = db.effective_access_level(&user, pid) {
                        if level > 0 {
                            active_project_id = pid;
                        }
                    }
                }
            }
        }

        let visible_projects = db.list_visible_projects(&user).unwrap_or_default();

        Ok(AuthContext {
            user,
            active_project_id,
            visible_projects,
        })
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

    #[test]
    fn test_auth_legacy_md5_fallback() {
        assert_eq!(md5_hex("root"), "63a9f0ea7bb98050796b649e85481845");
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
        assert!(verify_password("root", "63a9f0ea7bb98050796b649e85481845"));
        assert!(verify_password("root", "63A9F0EA7BB98050796B649E85481845"));
        assert!(!verify_password("wrong", "63a9f0ea7bb98050796b649e85481845"));
    }
}
