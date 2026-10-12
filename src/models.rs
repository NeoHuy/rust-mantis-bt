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

// ─── Sub-project 2: Issue Tracking ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum BugStatus {
    New = 10,
    Feedback = 20,
    Acknowledged = 30,
    Confirmed = 40,
    Assigned = 50,
    Resolved = 80,
    Closed = 90,
}

impl BugStatus {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            10 => Some(Self::New),
            20 => Some(Self::Feedback),
            30 => Some(Self::Acknowledged),
            40 => Some(Self::Confirmed),
            50 => Some(Self::Assigned),
            80 => Some(Self::Resolved),
            90 => Some(Self::Closed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Feedback => "feedback",
            Self::Acknowledged => "acknowledged",
            Self::Confirmed => "confirmed",
            Self::Assigned => "assigned",
            Self::Resolved => "resolved",
            Self::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum BugPriority {
    None = 10,
    Low = 20,
    Normal = 30,
    High = 40,
    Urgent = 50,
    Immediate = 60,
}

impl BugPriority {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            10 => Some(Self::None),
            20 => Some(Self::Low),
            30 => Some(Self::Normal),
            40 => Some(Self::High),
            50 => Some(Self::Urgent),
            60 => Some(Self::Immediate),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Urgent => "urgent",
            Self::Immediate => "immediate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum BugSeverity {
    Feature = 10,
    Trivial = 20,
    Text = 30,
    Tweak = 40,
    Minor = 50,
    Major = 60,
    Crash = 70,
    Block = 80,
}

impl BugSeverity {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            10 => Some(Self::Feature),
            20 => Some(Self::Trivial),
            30 => Some(Self::Text),
            40 => Some(Self::Tweak),
            50 => Some(Self::Minor),
            60 => Some(Self::Major),
            70 => Some(Self::Crash),
            80 => Some(Self::Block),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Trivial => "trivial",
            Self::Text => "text",
            Self::Tweak => "tweak",
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Crash => "crash",
            Self::Block => "block",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum BugReproducibility {
    Always = 10,
    Sometimes = 30,
    Random = 50,
    HaveNotTried = 70,
    UnableToReproduce = 90,
    NA = 100,
}

impl BugReproducibility {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            10 => Some(Self::Always),
            30 => Some(Self::Sometimes),
            50 => Some(Self::Random),
            70 => Some(Self::HaveNotTried),
            90 => Some(Self::UnableToReproduce),
            100 => Some(Self::NA),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Sometimes => "sometimes",
            Self::Random => "random",
            Self::HaveNotTried => "have not tried",
            Self::UnableToReproduce => "unable to reproduce",
            Self::NA => "N/A",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum BugResolution {
    Open = 10,
    Fixed = 20,
    Reopened = 30,
    UnableToReproduce = 40,
    NotFixable = 50,
    Duplicate = 60,
    NotABug = 70,
    Suspended = 80,
    WontFix = 90,
}

impl BugResolution {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            10 => Some(Self::Open),
            20 => Some(Self::Fixed),
            30 => Some(Self::Reopened),
            40 => Some(Self::UnableToReproduce),
            50 => Some(Self::NotFixable),
            60 => Some(Self::Duplicate),
            70 => Some(Self::NotABug),
            80 => Some(Self::Suspended),
            90 => Some(Self::WontFix),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Fixed => "fixed",
            Self::Reopened => "reopened",
            Self::UnableToReproduce => "unable to reproduce",
            Self::NotFixable => "not fixable",
            Self::Duplicate => "duplicate",
            Self::NotABug => "not a bug",
            Self::Suspended => "suspended",
            Self::WontFix => "won't fix",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum RelationshipType {
    RelatedTo = 0,
    DuplicateOf = 1,
    HasDuplicate = 2,
    Blocks = 3,
    BlockedBy = 4,
    ParentOf = 5,
    ChildOf = 6,
}

impl RelationshipType {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::RelatedTo),
            1 => Some(Self::DuplicateOf),
            2 => Some(Self::HasDuplicate),
            3 => Some(Self::Blocks),
            4 => Some(Self::BlockedBy),
            5 => Some(Self::ParentOf),
            6 => Some(Self::ChildOf),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RelatedTo => "related to",
            Self::DuplicateOf => "duplicate of",
            Self::HasDuplicate => "has duplicate",
            Self::Blocks => "blocks",
            Self::BlockedBy => "blocked by",
            Self::ParentOf => "parent of",
            Self::ChildOf => "child of",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bug {
    pub id: i64,
    pub project_id: i64,
    pub reporter_id: i64,
    pub handler_id: i64,
    pub duplicate_id: i64,
    pub priority: i32,
    pub severity: i32,
    pub reproducibility: i32,
    pub status: i32,
    pub resolution: i32,
    pub category_id: i64,
    pub date_submitted: i64,
    pub last_updated: i64,
    pub os: String,
    pub os_build: String,
    pub platform: String,
    pub version: String,
    pub fixed_in_version: String,
    pub target_version: String,
    pub build: String,
    pub summary: String,
    pub view_state: i32,
    pub sticky: i32,
    pub due_date: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugFull {
    pub bug: Bug,
    pub description: String,
    pub steps_to_reproduce: String,
    pub additional_information: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bugnote {
    pub id: i64,
    pub bug_id: i64,
    pub reporter_id: i64,
    pub bugnote_text_id: i64,
    pub view_state: i32,
    pub date_submitted: i64,
    pub last_modified: i64,
    pub note_type: i32,
    pub note_attr: String,
    pub time_tracking: i32,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugHistory {
    pub id: i64,
    pub user_id: i64,
    pub bug_id: i64,
    pub date_modified: i64,
    pub field_name: String,
    pub old_value: String,
    pub new_value: String,
    pub type_: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugRelationship {
    pub id: i64,
    pub source_bug_id: i64,
    pub destination_bug_id: i64,
    pub relationship_type: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugFile {
    pub id: i64,
    pub bug_id: i64,
    pub title: String,
    pub description: String,
    pub diskfile: String,
    pub filename: String,
    pub folder: String,
    pub filesize: i64,
    pub file_type: String,
    pub date_added: i64,
    pub user_id: i64,
}

/// Fields that can be updated on a bug. Only `Some` fields are written to DB.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BugUpdate {
    pub summary: Option<String>,
    pub category_id: Option<i64>,
    pub priority: Option<i32>,
    pub severity: Option<i32>,
    pub reproducibility: Option<i32>,
    pub version: Option<String>,
    pub target_version: Option<String>,
    pub fixed_in_version: Option<String>,
    pub view_state: Option<i32>,
    pub handler_id: Option<i64>,
    pub description: Option<String>,
    pub steps_to_reproduce: Option<String>,
    pub additional_information: Option<String>,
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
