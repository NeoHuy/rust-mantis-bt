use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum AccessLevel {
    Viewer = 10,
    Reporter = 25,
    Updater = 40,
    Developer = 55,
    Manager = 70,
    Administrator = 90,
    Nobody = 100,
}

impl AccessLevel {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            10 => Some(Self::Viewer),
            25 => Some(Self::Reporter),
            40 => Some(Self::Updater),
            55 => Some(Self::Developer),
            70 => Some(Self::Manager),
            90 => Some(Self::Administrator),
            100 => Some(Self::Nobody),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Reporter => "reporter",
            Self::Updater => "updater",
            Self::Developer => "developer",
            Self::Manager => "manager",
            Self::Administrator => "administrator",
            Self::Nobody => "nobody",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub realname: String,
    pub email: String,
    pub password: String,
    pub enabled: i32,
    pub protected: i32,
    pub access_level: i32,
    pub login_count: i32,
    pub lost_password_request_count: i32,
    pub failed_login_count: i32,
    pub cookie_string: String,
    pub last_visit: i64,
    pub date_created: i64,
}
