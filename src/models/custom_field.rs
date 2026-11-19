use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum CustomFieldType {
    String = 0,
    Numeric = 1,
    Float = 2,
    Enum = 3,
    Email = 4,
    Checkbox = 5,
    List = 6,
    Multilist = 7,
    Date = 8,
    Radio = 9,
    Textarea = 10,
}

impl CustomFieldType {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::String),
            1 => Some(Self::Numeric),
            2 => Some(Self::Float),
            3 => Some(Self::Enum),
            4 => Some(Self::Email),
            5 => Some(Self::Checkbox),
            6 => Some(Self::List),
            7 => Some(Self::Multilist),
            8 => Some(Self::Date),
            9 => Some(Self::Radio),
            10 => Some(Self::Textarea),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Numeric => "numeric",
            Self::Float => "float",
            Self::Enum => "enumeration",
            Self::Email => "email",
            Self::Checkbox => "checkbox",
            Self::List => "list",
            Self::Multilist => "multiselection list",
            Self::Date => "date",
            Self::Radio => "radio",
            Self::Textarea => "textarea",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomField {
    pub id: i64,
    pub name: String,
    pub field_type: i32,
    pub possible_values: String,
    pub default_value: String,
    pub valid_regexp: String,
    pub access_level_r: i32,
    pub access_level_rw: i32,
    pub length_min: i32,
    pub length_max: i32,
    pub require_report: i32,
    pub require_update: i32,
    pub display_report: i32,
    pub display_update: i32,
    pub require_resolved: i32,
    pub display_resolved: i32,
    pub display_closed: i32,
    pub require_closed: i32,
    pub filter_by: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomFieldValue {
    pub field_id: i64,
    pub field_name: String,
    pub field_type: i32,
    pub bug_id: i64,
    pub value: String,
}
