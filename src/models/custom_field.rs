use crate::models::{Category, ProjectVersion};
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
    #[serde(default)]
    pub access_level_r: i32,
    #[serde(default)]
    pub access_level_rw: i32,
}

/// Resolves dynamic tokens (`=versions`, `=released_versions`, `=future_versions`, `=categories`)
/// or splits static pipe-separated values.
pub fn resolve_possible_values(
    raw_possible_values: &str,
    versions: &[ProjectVersion],
    categories: &[Category],
) -> Vec<String> {
    let cat_names: Vec<String> = categories.iter().map(|c| c.name.clone()).collect();
    resolve_possible_values_strings(raw_possible_values, versions, &cat_names)
}

pub fn resolve_possible_values_strings(
    raw_possible_values: &str,
    versions: &[ProjectVersion],
    category_names: &[String],
) -> Vec<String> {
    let trimmed = raw_possible_values.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    match trimmed {
        "=versions" => versions
            .iter()
            .filter(|v| v.obsolete == 0)
            .map(|v| v.version.clone())
            .collect(),
        "=released_versions" => versions
            .iter()
            .filter(|v| v.released != 0 && v.obsolete == 0)
            .map(|v| v.version.clone())
            .collect(),
        "=future_versions" => versions
            .iter()
            .filter(|v| v.released == 0 && v.obsolete == 0)
            .map(|v| v.version.clone())
            .collect(),
        "=categories" => category_names.to_vec(),
        _ => trimmed
            .split('|')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

/// Checks if a string represents a valid YYYY-MM-DD date.
pub fn is_valid_ymd(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    if parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let year: i32 = match parts[0].parse() {
        Ok(y) => y,
        Err(_) => return false,
    };
    let month: u32 = match parts[1].parse() {
        Ok(m) if (1..=12).contains(&m) => m,
        _ => return false,
    };
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let max_days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    matches!(parts[2].parse::<u32>(), Ok(d) if (1..=max_days).contains(&d))
}

/// Converts a Unix timestamp in seconds to YYYY-MM-DD format using pure Rust.
pub fn timestamp_to_ymd(timestamp: i64) -> Option<String> {
    if timestamp < 0 {
        return None;
    }
    let days = timestamp / 86400;
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let final_y = if m <= 2 { y + 1 } else { y };
    Some(format!("{:04}-{:02}-{:02}", final_y, m, d))
}

/// Compiles a regular expression pattern with a 1 MiB compiled size limit.
/// The `regex` crate guarantees linear-time execution; the size limit bounds
/// memory and compilation time (defending against compile-time / memory DoS).
pub fn compile_custom_field_regex(pattern: &str) -> Result<regex::Regex, String> {
    regex::RegexBuilder::new(pattern)
        .size_limit(1 << 20)
        .build()
        .map_err(|e| {
            let err_str = e.to_string();
            err_str
                .lines()
                .find(|l| l.trim_start().starts_with("error:"))
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| {
                    err_str
                        .lines()
                        .next()
                        .unwrap_or("syntax error")
                        .trim()
                        .to_string()
                })
        })
}

/// Validates a custom field regular expression pattern.
/// Returns `Ok(())` if empty or whitespace-only, otherwise ensures it compiles cleanly.
pub fn validate_regex_pattern(pattern: &str) -> Result<(), String> {
    let p = pattern.trim();
    if p.is_empty() {
        return Ok(());
    }
    compile_custom_field_regex(p).map(|_| ())
}

/// Validates custom field value against its definition, length constraints,
/// required status, and resolved options.
pub fn validate_custom_field_value(
    cf: &CustomField,
    value: &str,
    resolved_possible_values: &[String],
    is_required: bool,
) -> Result<(), String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        if is_required {
            return Err(format!("Custom field '{}' is required", cf.name));
        }
        // If value is empty and not required, allow clearing it without triggering type/min-length errors.
        return Ok(());
    }

    // Min and max length checks for non-empty values
    let char_count = value.chars().count() as i32;
    if cf.length_min > 0 && char_count < cf.length_min {
        return Err(format!(
            "Custom field '{}' requires at least {} characters",
            cf.name, cf.length_min
        ));
    }
    if cf.length_max > 0 && char_count > cf.length_max {
        return Err(format!(
            "Custom field '{}' allows at most {} characters",
            cf.name, cf.length_max
        ));
    }

    match CustomFieldType::from_i32(cf.field_type) {
        Some(CustomFieldType::String) | Some(CustomFieldType::Textarea) => Ok(()),
        Some(CustomFieldType::Numeric) => match trimmed.parse::<i64>() {
            Ok(_) => Ok(()),
            Err(_) => Err(format!("Custom field '{}' must be an integer", cf.name)),
        },
        Some(CustomFieldType::Float) => match trimmed.parse::<f64>() {
            Ok(f) if f.is_finite() => Ok(()),
            _ => Err(format!("Custom field '{}' must be a valid number", cf.name)),
        },
        Some(CustomFieldType::Email) => {
            if trimmed.contains(char::is_whitespace) {
                return Err(format!(
                    "Custom field '{}' must be a valid email address",
                    cf.name
                ));
            }
            let parts: Vec<&str> = trimmed.split('@').collect();
            if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
                return Err(format!(
                    "Custom field '{}' must be a valid email address",
                    cf.name
                ));
            }
            let domain_parts: Vec<&str> = parts[1].split('.').collect();
            if domain_parts.len() < 2 || domain_parts.iter().any(|p| p.is_empty()) {
                return Err(format!(
                    "Custom field '{}' must be a valid email address",
                    cf.name
                ));
            }
            Ok(())
        }
        Some(CustomFieldType::Date) => {
            if is_valid_ymd(trimmed) {
                return Ok(());
            }
            if let Ok(ts) = trimmed.parse::<i64>() {
                if ts >= 0 {
                    return Ok(());
                }
            }
            Err(format!(
                "Custom field '{}' must be a valid date (YYYY-MM-DD or timestamp)",
                cf.name
            ))
        }
        Some(CustomFieldType::Enum)
        | Some(CustomFieldType::List)
        | Some(CustomFieldType::Radio) => {
            if resolved_possible_values.is_empty() {
                return Err(format!(
                    "No options available for custom field '{}'",
                    cf.name
                ));
            }
            if !resolved_possible_values.iter().any(|opt| opt == trimmed) {
                return Err(format!(
                    "Invalid value '{}' for custom field '{}'",
                    trimmed, cf.name
                ));
            }
            Ok(())
        }
        Some(CustomFieldType::Checkbox) => {
            if resolved_possible_values.is_empty() {
                if trimmed != "1" && !trimmed.eq_ignore_ascii_case("true") {
                    return Err(format!(
                        "Invalid value '{}' for custom field '{}'",
                        trimmed, cf.name
                    ));
                }
            } else {
                let items: Vec<&str> = trimmed
                    .split('|')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                for item in items {
                    if !resolved_possible_values.iter().any(|opt| opt == item) {
                        return Err(format!(
                            "Invalid value '{}' for custom field '{}'",
                            item, cf.name
                        ));
                    }
                }
            }
            Ok(())
        }
        Some(CustomFieldType::Multilist) => {
            if resolved_possible_values.is_empty() {
                return Err(format!(
                    "No options available for custom field '{}'",
                    cf.name
                ));
            }
            let items: Vec<&str> = trimmed
                .split('|')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .collect();
            for item in items {
                if !resolved_possible_values.iter().any(|opt| opt == item) {
                    return Err(format!(
                        "Invalid value '{}' for custom field '{}'",
                        item, cf.name
                    ));
                }
            }
            Ok(())
        }
        None => Err(format!("Unknown custom field type: {}", cf.field_type)),
    }?;

    if !cf.valid_regexp.trim().is_empty() {
        let re = compile_custom_field_regex(cf.valid_regexp.trim())
            .map_err(|e| format!("Custom field '{}' has invalid pattern: {}", cf.name, e))?;
        if !re.is_match(trimmed) {
            return Err(format!(
                "Custom field '{}' does not match required pattern",
                cf.name
            ));
        }
    }

    Ok(())
}
