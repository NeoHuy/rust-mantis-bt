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

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

pub fn constant_time_eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    a_bytes
        .iter()
        .zip(b_bytes.iter())
        .fold(0u8, |acc, (&x, &y)| {
            acc | (x.to_ascii_lowercase() ^ y.to_ascii_lowercase())
        })
        == 0
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
    constant_time_eq_ignore_ascii_case(&computed, expected_hash)
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
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let k: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613,
        0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193,
        0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d,
        0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942, 0x8771f681, 0x6d9d6122,
        0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa,
        0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244,
        0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb,
        0xeb86d391,
    ];

    for chunk in msg.as_chunks::<64>().0 {
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

    if stored.len() == 32
        && stored.chars().all(|c| c.is_ascii_hexdigit())
        && constant_time_eq_ignore_ascii_case(&md5_hex(plain), stored)
    {
        return true;
    }

    // Plaintext fallback or legacy comparison
    constant_time_eq(plain.as_bytes(), stored.as_bytes())
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

pub fn make_cookie_header(
    name: &str,
    value: &str,
    http_only: bool,
    max_age: Option<i64>,
) -> String {
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

pub fn generate_api_token() -> String {
    generate_cookie_string()
}

pub fn hash_api_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.trim().as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[derive(Clone, Debug)]
pub struct ApiAuth {
    pub user: crate::models::User,
}

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for ApiAuth
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
        use axum::http::StatusCode;
        use axum::response::IntoResponse;

        let db = crate::db::Db::from_ref(state);
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim())
            .unwrap_or("");

        let raw_token = if let Some(stripped) = auth_header.strip_prefix("Bearer ") {
            stripped.trim()
        } else if let Some(stripped) = auth_header.strip_prefix("bearer ") {
            stripped.trim()
        } else {
            auth_header
        };

        if raw_token.is_empty() {
            return Err((
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "message": "API token required" })),
            )
                .into_response());
        }

        match db.authenticate_api_token(raw_token) {
            Ok(Some(user)) => Ok(ApiAuth { user }),
            _ => Err((
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({ "message": "Invalid or revoked API token" })),
            )
                .into_response()),
        }
    }
}

#[cfg(test)]
#[path = "auth_test.rs"]
mod tests;
