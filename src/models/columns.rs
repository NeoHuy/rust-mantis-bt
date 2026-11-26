use crate::models::CustomField;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColumnContext {
    View,
    Print,
    Csv,
    Excel,
}

impl ColumnContext {
    pub fn config_id(&self) -> &'static str {
        match self {
            Self::View => "view_issues_page_columns",
            Self::Print => "print_issues_page_columns",
            Self::Csv => "csv_columns",
            Self::Excel => "excel_columns",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::View => "View Issues Columns",
            Self::Print => "Print Issues Columns",
            Self::Csv => "CSV Export Columns",
            Self::Excel => "Excel Export Columns",
        }
    }

    pub fn all() -> [Self; 4] {
        [Self::View, Self::Print, Self::Csv, Self::Excel]
    }
}

/// Ordered allowlist of built-in issue columns: `(column_id, display_label)`.
pub const STANDARD_COLUMNS: &[(&str, &str)] = &[
    ("selection", "Selection"),
    ("edit", "Edit"),
    ("priority", "Priority"),
    ("id", "Id"),
    ("project_id", "Project"),
    ("reporter_id", "Reporter"),
    ("handler_id", "Assigned To"),
    ("bugnotes_count", "Notes"),
    ("attachment_count", "Attachments"),
    ("category_id", "Category"),
    ("severity", "Severity"),
    ("reproducibility", "Reproducibility"),
    ("status", "Status"),
    ("resolution", "Resolution"),
    ("view_state", "View Status"),
    ("version", "Product Version"),
    ("target_version", "Target Version"),
    ("fixed_in_version", "Fixed in Version"),
    ("build", "Build"),
    ("platform", "Platform"),
    ("os", "OS"),
    ("os_build", "OS Build"),
    ("date_submitted", "Date Submitted"),
    ("last_updated", "Updated"),
    ("due_date", "Due Date"),
    ("sticky", "Sticky"),
    ("tags", "Tags"),
    ("summary", "Summary"),
    ("description", "Description"),
    ("steps_to_reproduce", "Steps To Reproduce"),
    ("additional_information", "Additional Information"),
];

pub fn is_standard_column(col: &str) -> bool {
    STANDARD_COLUMNS.iter().any(|(id, _)| *id == col)
}

pub fn standard_column_label(col: &str) -> Option<&'static str> {
    STANDARD_COLUMNS
        .iter()
        .find(|(id, _)| *id == col)
        .map(|(_, label)| *label)
}

pub fn parse_custom_field_column_id(col: &str) -> Option<i64> {
    col.strip_prefix("custom_field_")
        .and_then(|rest| rest.parse::<i64>().ok())
        .filter(|&id| id > 0)
}

pub fn custom_field_column_id(field_id: i64) -> String {
    format!("custom_field_{}", field_id)
}

pub fn default_columns_for_context(ctx: ColumnContext, project_id: i64) -> Vec<String> {
    let raw: Vec<&str> = match ctx {
        ColumnContext::View => {
            if project_id == 0 {
                vec![
                    "selection",
                    "edit",
                    "priority",
                    "id",
                    "bugnotes_count",
                    "attachment_count",
                    "project_id",
                    "category_id",
                    "severity",
                    "status",
                    "last_updated",
                    "summary",
                ]
            } else {
                vec![
                    "selection",
                    "edit",
                    "priority",
                    "id",
                    "bugnotes_count",
                    "attachment_count",
                    "category_id",
                    "severity",
                    "status",
                    "last_updated",
                    "summary",
                ]
            }
        }
        ColumnContext::Print => {
            if project_id == 0 {
                vec![
                    "id",
                    "project_id",
                    "priority",
                    "severity",
                    "status",
                    "handler_id",
                    "category_id",
                    "last_updated",
                    "summary",
                ]
            } else {
                vec![
                    "id",
                    "priority",
                    "severity",
                    "status",
                    "handler_id",
                    "category_id",
                    "last_updated",
                    "summary",
                ]
            }
        }
        ColumnContext::Csv => vec![
            "id",
            "project_id",
            "reporter_id",
            "handler_id",
            "priority",
            "severity",
            "reproducibility",
            "status",
            "resolution",
            "category_id",
            "summary",
            "last_updated",
        ],
        ColumnContext::Excel => vec![
            "id",
            "project_id",
            "reporter_id",
            "handler_id",
            "priority",
            "severity",
            "reproducibility",
            "version",
            "category_id",
            "date_submitted",
            "os",
            "os_build",
            "platform",
            "view_state",
            "last_updated",
            "summary",
            "status",
            "resolution",
            "fixed_in_version",
        ],
    };
    raw.into_iter().map(String::from).collect()
}

/// Parses comma-, whitespace-, or newline-separated column IDs into deduplicated tokens.
pub fn parse_columns_input(raw: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for token in raw.split(|c: char| c == ',' || c.is_whitespace()) {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if seen.insert(lower.clone()) {
            out.push(lower);
        }
    }
    out
}

/// Neutralizes spreadsheet formula-injection prefixes (`=`, `+`, `-`, `@`, `\t`, `\r`)
/// by prepending a single quote `'`.
pub fn sanitize_spreadsheet_cell(val: &str) -> String {
    let trimmed = val.trim_start();
    let needs_prefix = val.starts_with('\t')
        || val.starts_with('\r')
        || trimmed.starts_with('=')
        || trimmed.starts_with('+')
        || trimmed.starts_with('-')
        || trimmed.starts_with('@');
    if needs_prefix {
        format!("'{}", val)
    } else {
        val.to_string()
    }
}

/// Validates user-submitted column IDs against the strict standard column allowlist
/// and the user's accessible custom fields.
pub fn validate_columns_list(
    raw_cols: &[String],
    accessible_custom_fields: &[CustomField],
) -> Result<Vec<String>, String> {
    if raw_cols.is_empty() {
        return Err("At least one column must be selected.".to_string());
    }

    let cf_by_id: HashMap<i64, &CustomField> =
        accessible_custom_fields.iter().map(|c| (c.id, c)).collect();
    let cf_by_name: HashMap<String, i64> = accessible_custom_fields
        .iter()
        .map(|c| (c.name.trim().to_ascii_lowercase(), c.id))
        .collect();

    let mut seen = HashSet::new();
    let mut validated = Vec::new();
    let mut invalid = Vec::new();

    for col in raw_cols {
        let normalized = col.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if is_standard_column(&normalized) {
            if seen.insert(normalized.clone()) {
                validated.push(normalized);
            }
        } else if let Some(cf_id) = parse_custom_field_column_id(&normalized) {
            if cf_by_id.contains_key(&cf_id) {
                let canonical = custom_field_column_id(cf_id);
                if seen.insert(canonical.clone()) {
                    validated.push(canonical);
                }
            } else {
                invalid.push(col.clone());
            }
        } else if let Some(cf_name) = normalized.strip_prefix("custom_") {
            if let Some(&cf_id) = cf_by_name.get(cf_name) {
                let canonical = custom_field_column_id(cf_id);
                if seen.insert(canonical.clone()) {
                    validated.push(canonical);
                }
            } else {
                invalid.push(col.clone());
            }
        } else {
            invalid.push(col.clone());
        }
    }

    if !invalid.is_empty() {
        return Err(format!(
            "Invalid or inaccessible column(s): {}",
            invalid.join(", ")
        ));
    }

    if validated.is_empty() {
        return Err("At least one valid column must be selected.".to_string());
    }

    Ok(validated)
}

/// Filters stored columns against the standard allowlist and accessible custom fields
/// (used at read/render time so stale or unauthorized custom fields are dropped silently).
pub fn filter_authorized_columns(
    stored: &[String],
    accessible_custom_field_ids: &HashSet<i64>,
) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for col in stored {
        let norm = col.trim().to_ascii_lowercase();
        if is_standard_column(&norm) {
            if seen.insert(norm.clone()) {
                out.push(norm);
            }
        } else if let Some(cf_id) = parse_custom_field_column_id(&norm) {
            if accessible_custom_field_ids.contains(&cf_id) {
                let canonical = custom_field_column_id(cf_id);
                if seen.insert(canonical.clone()) {
                    out.push(canonical);
                }
            }
        }
    }
    out
}
