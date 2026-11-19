use serde::{Deserialize, Serialize};

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
