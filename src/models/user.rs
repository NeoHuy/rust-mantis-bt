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

    pub fn label_for_level(value: i32) -> Option<&'static str> {
        if let Some(level) = Self::from_i32(value) {
            return Some(level.as_str());
        }
        match value {
            v if v >= 100 => Some("nobody"),
            v if v >= 90 => Some("administrator"),
            v if v >= 70 => Some("manager"),
            v if v >= 55 => Some("developer"),
            v if v >= 40 => Some("updater"),
            v if v >= 25 => Some("reporter"),
            v if v >= 10 => Some("viewer"),
            _ => None,
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
