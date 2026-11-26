use crate::models::bug::Bug;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFilter {
    pub id: i64,
    pub user_id: i64,
    pub project_id: i64,
    pub is_public: i32,
    pub name: String,
    pub filter_string: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugFilter {
    pub project_id: Option<i64>,
    pub search: Option<String>,
    pub status: Option<i32>,
    pub hide_status: Option<i32>,
    pub priority: Option<i32>,
    pub severity: Option<i32>,
    pub category_id: Option<i64>,
    pub reporter_id: Option<i64>,
    pub handler_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub monitored_by: Option<i64>,
    #[serde(default)]
    pub target_version: Option<String>,
    #[serde(default)]
    pub fixed_in_version: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub per_page: Option<usize>,
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct ListBugsQuery {
    pub page: Option<i64>,
    pub filter_id: Option<i64>,
    pub project_id: Option<i64>,
    pub q: Option<String>,
    pub search: Option<String>,
    pub status: Option<i32>,
    pub hide_status: Option<i32>,
    pub priority: Option<i32>,
    pub severity: Option<i32>,
    pub category_id: Option<i64>,
    pub reporter_id: Option<i64>,
    pub handler_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub monitored_by: Option<i64>,
    pub target_version: Option<String>,
    pub fixed_in_version: Option<String>,
    pub msg: Option<String>,
    pub err: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_lenient_per_page")]
    pub per_page: Option<usize>,
}

fn deserialize_lenient_per_page<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct LenientVisitor;

    impl<'de> serde::de::Visitor<'de> for LenientVisitor {
        type Value = Option<usize>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("an optional integer or string representing per_page")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(self)
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(Some(v as usize))
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v > 0 {
                Ok(Some(v as usize))
            } else {
                Ok(None)
            }
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(v.parse::<usize>().ok())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_option(LenientVisitor)
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

pub fn clamp_per_page(val: Option<usize>) -> usize {
    match val {
        Some(10) => 10,
        Some(25) => 25,
        Some(50) => 50,
        Some(100) => 100,
        _ => 25,
    }
}

pub fn build_view_all_bug_page_url(q: &ListBugsQuery, page: i64) -> String {
    let mut params = Vec::new();
    params.push(format!("page={}", page.max(1)));

    if let Some(fid) = q.filter_id {
        if fid > 0 {
            params.push(format!("filter_id={}", fid));
        }
    }
    if let Some(ref q_val) = q.q {
        if !q_val.trim().is_empty() {
            params.push(format!("q={}", url_encode(q_val)));
        }
    }
    if let Some(ref s_val) = q.search {
        if !s_val.trim().is_empty() && q.q.as_deref().map(|s| s.trim()) != Some(s_val.trim()) {
            params.push(format!("search={}", url_encode(s_val)));
        }
    }
    if let Some(status) = q.status {
        if status > 0 {
            params.push(format!("status={}", status));
        }
    }
    if let Some(hide_status) = q.hide_status {
        if hide_status > 0 {
            params.push(format!("hide_status={}", hide_status));
        }
    }
    if let Some(priority) = q.priority {
        if priority > 0 {
            params.push(format!("priority={}", priority));
        }
    }
    if let Some(severity) = q.severity {
        if severity > 0 {
            params.push(format!("severity={}", severity));
        }
    }
    if let Some(category_id) = q.category_id {
        if category_id > 0 {
            params.push(format!("category_id={}", category_id));
        }
    }
    if let Some(reporter_id) = q.reporter_id {
        if reporter_id > 0 {
            params.push(format!("reporter_id={}", reporter_id));
        }
    }
    if let Some(handler_id) = q.handler_id {
        if handler_id != 0 {
            params.push(format!("handler_id={}", handler_id));
        }
    }
    if let Some(tag_id) = q.tag_id {
        if tag_id > 0 {
            params.push(format!("tag_id={}", tag_id));
        }
    }
    if let Some(monitored_by) = q.monitored_by {
        if monitored_by > 0 {
            params.push(format!("monitored_by={}", monitored_by));
        }
    }
    if let Some(ref sort) = q.sort {
        if !sort.trim().is_empty() {
            params.push(format!("sort={}", url_encode(sort)));
        }
    }
    if let Some(ref dir) = q.dir {
        if !dir.trim().is_empty() {
            params.push(format!("dir={}", url_encode(dir)));
        }
    }
    if let Some(per_page) = q.per_page {
        if per_page > 0 {
            params.push(format!("per_page={}", clamp_per_page(Some(per_page))));
        }
    }

    format!("/view_all_bug_page?{}", params.join("&"))
}

pub fn build_csv_export_url(q: &ListBugsQuery) -> String {
    let mut params = Vec::new();

    if let Some(fid) = q.filter_id {
        if fid > 0 {
            params.push(format!("filter_id={}", fid));
        }
    }
    if let Some(ref q_val) = q.q {
        if !q_val.trim().is_empty() {
            params.push(format!("q={}", url_encode(q_val)));
        }
    }
    if let Some(ref s_val) = q.search {
        if !s_val.trim().is_empty() && q.q.as_deref().map(|s| s.trim()) != Some(s_val.trim()) {
            params.push(format!("search={}", url_encode(s_val)));
        }
    }
    if let Some(status) = q.status {
        if status > 0 {
            params.push(format!("status={}", status));
        }
    }
    if let Some(hide_status) = q.hide_status {
        if hide_status > 0 {
            params.push(format!("hide_status={}", hide_status));
        }
    }
    if let Some(priority) = q.priority {
        if priority > 0 {
            params.push(format!("priority={}", priority));
        }
    }
    if let Some(severity) = q.severity {
        if severity > 0 {
            params.push(format!("severity={}", severity));
        }
    }
    if let Some(category_id) = q.category_id {
        if category_id > 0 {
            params.push(format!("category_id={}", category_id));
        }
    }
    if let Some(reporter_id) = q.reporter_id {
        if reporter_id > 0 {
            params.push(format!("reporter_id={}", reporter_id));
        }
    }
    if let Some(handler_id) = q.handler_id {
        if handler_id != 0 {
            params.push(format!("handler_id={}", handler_id));
        }
    }
    if let Some(tag_id) = q.tag_id {
        if tag_id > 0 {
            params.push(format!("tag_id={}", tag_id));
        }
    }
    if let Some(monitored_by) = q.monitored_by {
        if monitored_by > 0 {
            params.push(format!("monitored_by={}", monitored_by));
        }
    }
    if let Some(ref sort) = q.sort {
        if !sort.trim().is_empty() {
            params.push(format!("sort={}", url_encode(sort)));
        }
    }
    if let Some(ref dir) = q.dir {
        if !dir.trim().is_empty() {
            params.push(format!("dir={}", url_encode(dir)));
        }
    }

    if params.is_empty() {
        "/csv_export".to_string()
    } else {
        format!("/csv_export?{}", params.join("&"))
    }
}

pub fn sort_bugs(bugs: &mut [Bug], sort_key: Option<&str>, dir: Option<&str>) {
    let is_asc = matches!(dir, Some(d) if d.eq_ignore_ascii_case("asc"));

    let key = match sort_key {
        Some("id") => "id",
        Some("priority") => "priority",
        Some("severity") => "severity",
        Some("status") => "status",
        Some("summary") => "summary",
        Some("last_updated") => "last_updated",
        _ => "last_updated",
    };

    bugs.sort_by(|a, b| {
        // Sticky always pinned at top: sticky == 1 comes before sticky == 0
        let sticky_cmp = b.sticky.cmp(&a.sticky);
        if sticky_cmp != std::cmp::Ordering::Equal {
            return sticky_cmp;
        }

        let order = match key {
            "id" => a.id.cmp(&b.id),
            "priority" => a.priority.cmp(&b.priority),
            "severity" => a.severity.cmp(&b.severity),
            "status" => a.status.cmp(&b.status),
            "summary" => {
                let cmp = a
                    .summary
                    .chars()
                    .flat_map(char::to_lowercase)
                    .cmp(b.summary.chars().flat_map(char::to_lowercase));
                if cmp == std::cmp::Ordering::Equal {
                    a.summary.cmp(&b.summary)
                } else {
                    cmp
                }
            }
            "last_updated" => a.last_updated.cmp(&b.last_updated),
            _ => a.last_updated.cmp(&b.last_updated),
        };

        if is_asc {
            order.then_with(|| a.id.cmp(&b.id))
        } else {
            order.reverse().then_with(|| b.id.cmp(&a.id))
        }
    });
}
