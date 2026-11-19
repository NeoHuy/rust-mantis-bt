use serde::{Deserialize, Serialize};

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
