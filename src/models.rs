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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ProjectStatus {
    Development = 10,
    Release = 30,
    Stable = 50,
    Obsolete = 70,
}

impl ProjectStatus {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            10 => Some(Self::Development),
            30 => Some(Self::Release),
            50 => Some(Self::Stable),
            70 => Some(Self::Obsolete),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Release => "release",
            Self::Stable => "stable",
            Self::Obsolete => "obsolete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum ViewState {
    Public = 10,
    Private = 50,
}

impl ViewState {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            10 => Some(Self::Public),
            50 => Some(Self::Private),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub status: i32,
    pub enabled: i32,
    pub view_state: i32,
    pub access_min: i32,
    pub description: String,
    pub category_id: i64,
    pub inherit_global: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectHierarchy {
    pub child_id: i64,
    pub parent_id: i64,
    pub inherit_parent: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectUser {
    pub project_id: i64,
    pub user_id: i64,
    pub access_level: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Category {
    pub id: i64,
    pub project_id: i64,
    pub user_id: i64,
    pub name: String,
    pub status: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectVersion {
    pub id: i64,
    pub project_id: i64,
    pub version: String,
    pub description: String,
    pub released: i32,
    pub obsolete: i32,
    pub date_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub config_id: String,
    pub project_id: i64,
    pub user_id: i64,
    pub access_reqd: i32,
    pub config_type: i32,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_level_conversion_and_strings() {
        assert_eq!(AccessLevel::from_i32(10), Some(AccessLevel::Viewer));
        assert_eq!(AccessLevel::from_i32(25), Some(AccessLevel::Reporter));
        assert_eq!(AccessLevel::from_i32(40), Some(AccessLevel::Updater));
        assert_eq!(AccessLevel::from_i32(55), Some(AccessLevel::Developer));
        assert_eq!(AccessLevel::from_i32(70), Some(AccessLevel::Manager));
        assert_eq!(AccessLevel::from_i32(90), Some(AccessLevel::Administrator));
        assert_eq!(AccessLevel::from_i32(100), Some(AccessLevel::Nobody));
        assert_eq!(AccessLevel::from_i32(999), None);

        assert_eq!(AccessLevel::Viewer.as_str(), "viewer");
        assert_eq!(AccessLevel::Reporter.as_str(), "reporter");
        assert_eq!(AccessLevel::Updater.as_str(), "updater");
        assert_eq!(AccessLevel::Developer.as_str(), "developer");
        assert_eq!(AccessLevel::Manager.as_str(), "manager");
        assert_eq!(AccessLevel::Administrator.as_str(), "administrator");
        assert_eq!(AccessLevel::Nobody.as_str(), "nobody");

        assert!(AccessLevel::Administrator > AccessLevel::Manager);
        assert!(AccessLevel::Manager > AccessLevel::Developer);
    }

    #[test]
    fn test_project_status_conversion_and_strings() {
        assert_eq!(
            ProjectStatus::from_i32(10),
            Some(ProjectStatus::Development)
        );
        assert_eq!(ProjectStatus::from_i32(30), Some(ProjectStatus::Release));
        assert_eq!(ProjectStatus::from_i32(50), Some(ProjectStatus::Stable));
        assert_eq!(ProjectStatus::from_i32(70), Some(ProjectStatus::Obsolete));
        assert_eq!(ProjectStatus::from_i32(0), None);

        assert_eq!(ProjectStatus::Development.as_str(), "development");
        assert_eq!(ProjectStatus::Release.as_str(), "release");
        assert_eq!(ProjectStatus::Stable.as_str(), "stable");
        assert_eq!(ProjectStatus::Obsolete.as_str(), "obsolete");
    }

    #[test]
    fn test_view_state_conversion_and_strings() {
        assert_eq!(ViewState::from_i32(10), Some(ViewState::Public));
        assert_eq!(ViewState::from_i32(50), Some(ViewState::Private));
        assert_eq!(ViewState::from_i32(100), None);

        assert_eq!(ViewState::Public.as_str(), "public");
        assert_eq!(ViewState::Private.as_str(), "private");
    }
}
