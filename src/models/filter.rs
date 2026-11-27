use crate::models::bug::Bug;
use crate::models::custom_field::is_valid_ymd;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const FILTER_MATCH_ALL: i32 = 0;
pub const FILTER_MATCH_ANY: i32 = 1;

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
    pub commented_by: Option<i64>,
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
    #[serde(default)]
    pub match_type: i32,
    #[serde(default)]
    pub statuses: Vec<i32>,
    #[serde(default)]
    pub hide_statuses: Vec<i32>,
    #[serde(default)]
    pub priorities: Vec<i32>,
    #[serde(default)]
    pub severities: Vec<i32>,
    #[serde(default)]
    pub category_ids: Vec<i64>,
    #[serde(default)]
    pub reporter_ids: Vec<i64>,
    #[serde(default)]
    pub handler_ids: Vec<i64>,
    #[serde(default)]
    pub tag_ids: Vec<i64>,
    #[serde(default)]
    pub monitored_by_ids: Vec<i64>,
    #[serde(default)]
    pub resolutions: Vec<i32>,
    #[serde(default)]
    pub reproducibilities: Vec<i32>,
    #[serde(default)]
    pub view_state: Option<i32>,
    #[serde(default)]
    pub sticky: Option<i32>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub target_versions: Vec<String>,
    #[serde(default)]
    pub fixed_in_versions: Vec<String>,
    #[serde(default)]
    pub date_submitted_from: Option<String>,
    #[serde(default)]
    pub date_submitted_to: Option<String>,
    #[serde(default)]
    pub last_updated_from: Option<String>,
    #[serde(default)]
    pub last_updated_to: Option<String>,
    #[serde(default)]
    pub relationship_type: Option<i32>,
    #[serde(default)]
    pub relationship_bug_id: Option<i64>,
    #[serde(default)]
    pub note_user_ids: Vec<i64>,
    #[serde(default)]
    pub custom_fields: BTreeMap<i64, Vec<String>>,
}

fn merge_positive_i32(single: Option<i32>, multi: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    if let Some(v) = single.filter(|&x| x > 0) {
        out.push(v);
    }
    for &v in multi {
        if v > 0 && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

fn merge_positive_i64(single: Option<i64>, multi: &[i64]) -> Vec<i64> {
    let mut out = Vec::new();
    if let Some(v) = single.filter(|&x| x > 0) {
        out.push(v);
    }
    for &v in multi {
        if v > 0 && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

fn merge_handler_ids(single: Option<i64>, multi: &[i64]) -> Vec<i64> {
    let mut out = Vec::new();
    if let Some(v) = single.filter(|&x| x != 0) {
        out.push(v);
    }
    for &v in multi {
        if v != 0 && !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

fn dedup_non_empty_strings(items: &[String]) -> Vec<String> {
    merge_strings(None, items)
}

fn merge_strings(single: Option<&str>, multi: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(s) = single {
        let trimmed = s.trim();
        if !trimmed.is_empty() && trimmed != "[any]" {
            out.push(trimmed.to_string());
        }
    }
    for item in multi {
        let trimmed = item.trim();
        if trimmed.is_empty() || trimmed == "[any]" {
            continue;
        }
        let owned = trimmed.to_string();
        if !out.contains(&owned) {
            out.push(owned);
        }
    }
    out
}

impl BugFilter {
    pub fn effective_statuses(&self) -> Vec<i32> {
        merge_positive_i32(self.status, &self.statuses)
    }

    pub fn effective_hide_statuses(&self) -> Vec<i32> {
        merge_positive_i32(None, &self.hide_statuses)
    }

    pub fn effective_priorities(&self) -> Vec<i32> {
        merge_positive_i32(self.priority, &self.priorities)
    }

    pub fn effective_severities(&self) -> Vec<i32> {
        merge_positive_i32(self.severity, &self.severities)
    }

    pub fn effective_category_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.category_id, &self.category_ids)
    }

    pub fn effective_reporter_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.reporter_id, &self.reporter_ids)
    }

    pub fn effective_handler_ids(&self) -> Vec<i64> {
        merge_handler_ids(self.handler_id, &self.handler_ids)
    }

    pub fn effective_tag_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.tag_id, &self.tag_ids)
    }

    pub fn effective_monitored_by_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.monitored_by, &self.monitored_by_ids)
    }

    pub fn effective_resolutions(&self) -> Vec<i32> {
        merge_positive_i32(None, &self.resolutions)
    }

    pub fn effective_reproducibilities(&self) -> Vec<i32> {
        merge_positive_i32(None, &self.reproducibilities)
    }

    pub fn effective_versions(&self) -> Vec<String> {
        dedup_non_empty_strings(&self.versions)
    }

    pub fn effective_target_versions(&self) -> Vec<String> {
        merge_strings(self.target_version.as_deref(), &self.target_versions)
    }

    pub fn effective_fixed_in_versions(&self) -> Vec<String> {
        merge_strings(self.fixed_in_version.as_deref(), &self.fixed_in_versions)
    }

    pub fn effective_note_user_ids(&self) -> Vec<i64> {
        merge_positive_i64(None, &self.note_user_ids)
    }

    pub fn retain_custom_fields(&mut self, allowed_cf_ids: &HashSet<i64>) {
        self.custom_fields
            .retain(|fid, _| allowed_cf_ids.contains(fid));
    }

    pub fn to_list_query(&self) -> ListBugsQuery {
        let statuses = self.effective_statuses();
        let priorities = self.effective_priorities();
        let severities = self.effective_severities();
        let category_ids = self.effective_category_ids();
        let reporter_ids = self.effective_reporter_ids();
        let handler_ids = self.effective_handler_ids();
        let tag_ids = self.effective_tag_ids();
        let monitored_by_ids = self.effective_monitored_by_ids();

        ListBugsQuery {
            page: Some(1),
            filter_id: None,
            project_id: self.project_id.filter(|&v| v > 0),
            q: self.search.clone(),
            search: None,
            status: (statuses.len() == 1).then(|| statuses[0]),
            hide_status: self.hide_status.filter(|&v| v > 0),
            priority: (priorities.len() == 1).then(|| priorities[0]),
            severity: (severities.len() == 1).then(|| severities[0]),
            category_id: (category_ids.len() == 1).then(|| category_ids[0]),
            reporter_id: (reporter_ids.len() == 1).then(|| reporter_ids[0]),
            handler_id: (handler_ids.len() == 1).then(|| handler_ids[0]),
            tag_id: (tag_ids.len() == 1).then(|| tag_ids[0]),
            monitored_by: (monitored_by_ids.len() == 1).then(|| monitored_by_ids[0]),
            commented_by: self.commented_by.filter(|&v| v > 0),
            target_version: None,
            fixed_in_version: None,
            msg: None,
            err: None,
            sort: self.sort.clone(),
            dir: self.dir.clone(),
            per_page: self.per_page,
            match_type: (self.match_type == FILTER_MATCH_ANY).then_some(FILTER_MATCH_ANY),
            statuses: if statuses.len() > 1 {
                statuses
            } else {
                Vec::new()
            },
            hide_statuses: self.effective_hide_statuses(),
            priorities: if priorities.len() > 1 {
                priorities
            } else {
                Vec::new()
            },
            severities: if severities.len() > 1 {
                severities
            } else {
                Vec::new()
            },
            category_ids: if category_ids.len() > 1 {
                category_ids
            } else {
                Vec::new()
            },
            reporter_ids: if reporter_ids.len() > 1 {
                reporter_ids
            } else {
                Vec::new()
            },
            handler_ids: if handler_ids.len() > 1 {
                handler_ids
            } else {
                Vec::new()
            },
            tag_ids: if tag_ids.len() > 1 {
                tag_ids
            } else {
                Vec::new()
            },
            monitored_by_ids: if monitored_by_ids.len() > 1 {
                monitored_by_ids
            } else {
                Vec::new()
            },
            resolutions: self.effective_resolutions(),
            reproducibilities: self.effective_reproducibilities(),
            view_state: self.view_state.filter(|&v| v == 10 || v == 50),
            sticky: self.sticky.filter(|&v| v == 0 || v == 1),
            versions: self.effective_versions(),
            target_versions: self.effective_target_versions(),
            fixed_in_versions: self.effective_fixed_in_versions(),
            date_submitted_from: self
                .date_submitted_from
                .clone()
                .filter(|s| !s.trim().is_empty()),
            date_submitted_to: self
                .date_submitted_to
                .clone()
                .filter(|s| !s.trim().is_empty()),
            last_updated_from: self
                .last_updated_from
                .clone()
                .filter(|s| !s.trim().is_empty()),
            last_updated_to: self
                .last_updated_to
                .clone()
                .filter(|s| !s.trim().is_empty()),
            relationship_type: self.relationship_type,
            relationship_bug_id: self.relationship_bug_id.filter(|&v| v > 0),
            note_user_ids: self.effective_note_user_ids(),
            custom_fields: self.custom_fields.clone(),
        }
    }
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
    #[serde(default)]
    pub commented_by: Option<i64>,
    pub target_version: Option<String>,
    pub fixed_in_version: Option<String>,
    pub msg: Option<String>,
    pub err: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_lenient_per_page")]
    pub per_page: Option<usize>,
    #[serde(default)]
    pub match_type: Option<i32>,
    #[serde(default)]
    pub statuses: Vec<i32>,
    #[serde(default)]
    pub hide_statuses: Vec<i32>,
    #[serde(default)]
    pub priorities: Vec<i32>,
    #[serde(default)]
    pub severities: Vec<i32>,
    #[serde(default)]
    pub category_ids: Vec<i64>,
    #[serde(default)]
    pub reporter_ids: Vec<i64>,
    #[serde(default)]
    pub handler_ids: Vec<i64>,
    #[serde(default)]
    pub tag_ids: Vec<i64>,
    #[serde(default)]
    pub monitored_by_ids: Vec<i64>,
    #[serde(default)]
    pub resolutions: Vec<i32>,
    #[serde(default)]
    pub reproducibilities: Vec<i32>,
    #[serde(default)]
    pub view_state: Option<i32>,
    #[serde(default)]
    pub sticky: Option<i32>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub target_versions: Vec<String>,
    #[serde(default)]
    pub fixed_in_versions: Vec<String>,
    #[serde(default)]
    pub date_submitted_from: Option<String>,
    #[serde(default)]
    pub date_submitted_to: Option<String>,
    #[serde(default)]
    pub last_updated_from: Option<String>,
    #[serde(default)]
    pub last_updated_to: Option<String>,
    #[serde(default)]
    pub relationship_type: Option<i32>,
    #[serde(default)]
    pub relationship_bug_id: Option<i64>,
    #[serde(default)]
    pub note_user_ids: Vec<i64>,
    #[serde(default)]
    pub custom_fields: BTreeMap<i64, Vec<String>>,
}

pub const MAX_FILTER_VALUES_PER_FIELD: usize = 100;
pub const MAX_FILTER_CUSTOM_FIELDS: usize = 50;

fn push_unique_i32(vec: &mut Vec<i32>, val: i32, allow_neg_one: bool) {
    if vec.len() >= MAX_FILTER_VALUES_PER_FIELD {
        return;
    }
    if (val > 0 || (allow_neg_one && val == -1)) && !vec.contains(&val) {
        vec.push(val);
    }
}

fn push_unique_i64(vec: &mut Vec<i64>, val: i64, allow_neg_one: bool) {
    if vec.len() >= MAX_FILTER_VALUES_PER_FIELD {
        return;
    }
    if (val > 0 || (allow_neg_one && val == -1)) && !vec.contains(&val) {
        vec.push(val);
    }
}

fn parse_i32_tokens(raw: &str, out: &mut Vec<i32>, allow_neg_one: bool) {
    for part in raw.split(',') {
        if out.len() >= MAX_FILTER_VALUES_PER_FIELD {
            break;
        }
        if let Ok(v) = part.trim().parse::<i32>() {
            push_unique_i32(out, v, allow_neg_one);
        }
    }
}

fn parse_i64_tokens(raw: &str, out: &mut Vec<i64>, allow_neg_one: bool) {
    for part in raw.split(',') {
        if out.len() >= MAX_FILTER_VALUES_PER_FIELD {
            break;
        }
        if let Ok(v) = part.trim().parse::<i64>() {
            push_unique_i64(out, v, allow_neg_one);
        }
    }
}

fn parse_str_token(raw: &str, out: &mut Vec<String>) {
    if out.len() >= MAX_FILTER_VALUES_PER_FIELD {
        return;
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "[any]" {
        return;
    }
    let s = trimmed.to_string();
    if !out.contains(&s) {
        out.push(s);
    }
}

fn url_decode_lossy(s: &str) -> String {
    let raw = s.as_bytes();
    let mut bytes = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            b'+' => {
                bytes.push(b' ');
                i += 1;
            }
            b'%' => {
                if i + 2 < raw.len()
                    && raw[i + 1].is_ascii_hexdigit()
                    && raw[i + 2].is_ascii_hexdigit()
                {
                    if let Ok(s_hex) = std::str::from_utf8(&raw[i + 1..i + 3]) {
                        if let Ok(val) = u8::from_str_radix(s_hex, 16) {
                            bytes.push(val);
                            i += 3;
                            continue;
                        }
                    }
                }
                bytes.push(b'%');
                i += 1;
            }
            b => {
                bytes.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn parse_query_pairs(raw_query: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for part in raw_query.split('&') {
        if part.is_empty() {
            continue;
        }
        if let Some((k, v)) = part.split_once('=') {
            pairs.push((url_decode_lossy(k), url_decode_lossy(v)));
        } else {
            pairs.push((url_decode_lossy(part), String::new()));
        }
    }
    pairs
}

impl ListBugsQuery {
    pub fn from_raw_query(raw: Option<&str>) -> Self {
        match raw {
            Some(qs) if !qs.is_empty() => Self::from_urlencoded_pairs(&parse_query_pairs(qs)),
            _ => Self::default(),
        }
    }

    pub fn from_urlencoded_pairs(pairs: &[(String, String)]) -> Self {
        let mut q = Self::default();
        let mut statuses = Vec::new();
        let mut hide_statuses = Vec::new();
        let mut priorities = Vec::new();
        let mut severities = Vec::new();
        let mut category_ids = Vec::new();
        let mut reporter_ids = Vec::new();
        let mut handler_ids = Vec::new();
        let mut tag_ids = Vec::new();
        let mut monitored_by_ids = Vec::new();
        let mut resolutions = Vec::new();
        let mut reproducibilities = Vec::new();
        let mut note_user_ids = Vec::new();

        for (raw_key, val) in pairs {
            let key = raw_key.strip_suffix("[]").unwrap_or(raw_key.as_str());
            let trimmed = val.trim();
            match key {
                "page" => {
                    if let Ok(p) = trimmed.parse::<i64>() {
                        q.page = Some(p);
                    }
                }
                "commented_by" => {
                    if let Ok(uid) = trimmed.parse::<i64>() {
                        if uid > 0 {
                            q.commented_by = Some(uid);
                        }
                    }
                }
                "filter_id" => {
                    if let Ok(fid) = trimmed.parse::<i64>() {
                        q.filter_id = Some(fid);
                    }
                }
                "project_id" => {
                    if let Ok(pid) = trimmed.parse::<i64>() {
                        if pid > 0 {
                            q.project_id = Some(pid);
                        }
                    }
                }
                "q" => {
                    if !trimmed.is_empty() {
                        q.q = Some(val.clone());
                    }
                }
                "search" => {
                    if !trimmed.is_empty() {
                        q.search = Some(val.clone());
                    }
                }
                "msg" => {
                    if !trimmed.is_empty() {
                        q.msg = Some(val.clone());
                    }
                }
                "err" => {
                    if !trimmed.is_empty() {
                        q.err = Some(val.clone());
                    }
                }
                "sort" => {
                    if !trimmed.is_empty() {
                        q.sort = Some(trimmed.to_string());
                    }
                }
                "dir" => {
                    if !trimmed.is_empty() {
                        q.dir = Some(trimmed.to_string());
                    }
                }
                "per_page" => {
                    if let Ok(pp) = trimmed.parse::<usize>() {
                        if pp > 0 {
                            q.per_page = Some(pp);
                        }
                    }
                }
                "match_type" => {
                    let lower = trimmed.to_ascii_lowercase();
                    if lower == "1" || lower == "any" {
                        q.match_type = Some(FILTER_MATCH_ANY);
                    } else if lower == "0" || lower == "all" {
                        q.match_type = Some(FILTER_MATCH_ALL);
                    }
                }
                "status" | "statuses" => parse_i32_tokens(trimmed, &mut statuses, false),
                "hide_status" => {
                    if let Ok(v) = trimmed.parse::<i32>() {
                        if v > 0 {
                            q.hide_status = Some(v);
                        }
                    }
                }
                "hide_statuses" => parse_i32_tokens(trimmed, &mut hide_statuses, false),
                "priority" | "priorities" => parse_i32_tokens(trimmed, &mut priorities, false),
                "severity" | "severities" => parse_i32_tokens(trimmed, &mut severities, false),
                "category_id" | "category_ids" => {
                    parse_i64_tokens(trimmed, &mut category_ids, false)
                }
                "reporter_id" | "reporter_ids" => {
                    parse_i64_tokens(trimmed, &mut reporter_ids, false)
                }
                "handler_id" | "handler_ids" => parse_i64_tokens(trimmed, &mut handler_ids, true),
                "tag_id" | "tag_ids" => parse_i64_tokens(trimmed, &mut tag_ids, false),
                "monitored_by" | "monitored_by_ids" => {
                    parse_i64_tokens(trimmed, &mut monitored_by_ids, false)
                }
                "resolution" | "resolutions" => parse_i32_tokens(trimmed, &mut resolutions, false),
                "reproducibility" | "reproducibilities" => {
                    parse_i32_tokens(trimmed, &mut reproducibilities, false)
                }
                "view_state" => {
                    if let Ok(v) = trimmed.parse::<i32>() {
                        if v == 10 || v == 50 {
                            q.view_state = Some(v);
                        }
                    }
                }
                "sticky" => {
                    let lower = trimmed.to_ascii_lowercase();
                    if lower == "1" || lower == "yes" || lower == "true" {
                        q.sticky = Some(1);
                    } else if lower == "0" || lower == "no" || lower == "false" {
                        q.sticky = Some(0);
                    }
                }
                "version" | "versions" => parse_str_token(trimmed, &mut q.versions),
                "target_version" | "target_versions" => {
                    parse_str_token(trimmed, &mut q.target_versions)
                }
                "fixed_in_version" | "fixed_in_versions" => {
                    parse_str_token(trimmed, &mut q.fixed_in_versions)
                }
                "date_submitted_from" => {
                    if !trimmed.is_empty() {
                        q.date_submitted_from = Some(trimmed.to_string());
                    }
                }
                "date_submitted_to" => {
                    if !trimmed.is_empty() {
                        q.date_submitted_to = Some(trimmed.to_string());
                    }
                }
                "last_updated_from" => {
                    if !trimmed.is_empty() {
                        q.last_updated_from = Some(trimmed.to_string());
                    }
                }
                "last_updated_to" => {
                    if !trimmed.is_empty() {
                        q.last_updated_to = Some(trimmed.to_string());
                    }
                }
                "relationship_type" => {
                    if let Ok(rt) = trimmed.parse::<i32>() {
                        if (-1..=6).contains(&rt) {
                            q.relationship_type = Some(rt);
                        }
                    }
                }
                "relationship_bug_id" => {
                    if let Ok(bid) = trimmed.parse::<i64>() {
                        if bid > 0 {
                            q.relationship_bug_id = Some(bid);
                        }
                    }
                }
                "note_user_id" | "note_user_ids" => {
                    parse_i64_tokens(trimmed, &mut note_user_ids, false)
                }
                other => {
                    if let Some(id_str) = other.strip_prefix("custom_field_") {
                        if let Ok(fid) = id_str.parse::<i64>() {
                            if fid > 0
                                && !trimmed.is_empty()
                                && trimmed != "[any]"
                                && (q.custom_fields.contains_key(&fid)
                                    || q.custom_fields.len() < MAX_FILTER_CUSTOM_FIELDS)
                            {
                                let entry = q.custom_fields.entry(fid).or_default();
                                if entry.len() < MAX_FILTER_VALUES_PER_FIELD {
                                    let s = trimmed.to_string();
                                    if !entry.contains(&s) {
                                        entry.push(s);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if statuses.len() == 1 {
            q.status = Some(statuses[0]);
        } else {
            q.statuses = statuses;
        }
        q.hide_statuses = hide_statuses;
        if priorities.len() == 1 {
            q.priority = Some(priorities[0]);
        } else {
            q.priorities = priorities;
        }
        if severities.len() == 1 {
            q.severity = Some(severities[0]);
        } else {
            q.severities = severities;
        }
        if category_ids.len() == 1 {
            q.category_id = Some(category_ids[0]);
        } else {
            q.category_ids = category_ids;
        }
        if reporter_ids.len() == 1 {
            q.reporter_id = Some(reporter_ids[0]);
        } else {
            q.reporter_ids = reporter_ids;
        }
        if handler_ids.len() == 1 {
            q.handler_id = Some(handler_ids[0]);
        } else {
            q.handler_ids = handler_ids;
        }
        if tag_ids.len() == 1 {
            q.tag_id = Some(tag_ids[0]);
        } else {
            q.tag_ids = tag_ids;
        }
        if monitored_by_ids.len() == 1 {
            q.monitored_by = Some(monitored_by_ids[0]);
        } else {
            q.monitored_by_ids = monitored_by_ids;
        }
        q.resolutions = resolutions;
        q.reproducibilities = reproducibilities;
        q.note_user_ids = note_user_ids;

        q
    }

    pub fn effective_statuses(&self) -> Vec<i32> {
        merge_positive_i32(self.status, &self.statuses)
    }

    pub fn effective_priorities(&self) -> Vec<i32> {
        merge_positive_i32(self.priority, &self.priorities)
    }

    pub fn effective_severities(&self) -> Vec<i32> {
        merge_positive_i32(self.severity, &self.severities)
    }

    pub fn effective_category_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.category_id, &self.category_ids)
    }

    pub fn effective_reporter_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.reporter_id, &self.reporter_ids)
    }

    pub fn effective_handler_ids(&self) -> Vec<i64> {
        merge_handler_ids(self.handler_id, &self.handler_ids)
    }

    pub fn effective_tag_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.tag_id, &self.tag_ids)
    }

    pub fn effective_monitored_by_ids(&self) -> Vec<i64> {
        merge_positive_i64(self.monitored_by, &self.monitored_by_ids)
    }

    pub fn effective_target_versions(&self) -> Vec<String> {
        merge_strings(self.target_version.as_deref(), &self.target_versions)
    }

    pub fn effective_fixed_in_versions(&self) -> Vec<String> {
        merge_strings(self.fixed_in_version.as_deref(), &self.fixed_in_versions)
    }
}

/// Parses a `YYYY-MM-DD` date string (or Unix epoch seconds) into UTC timestamp bounds.
/// When `end_of_day` is true and input is `YYYY-MM-DD`, returns `23:59:59` of that UTC date.
pub fn parse_date_filter_bound(s: &str, end_of_day: bool) -> Option<i64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_valid_ymd(trimmed) {
        let parts: Vec<&str> = trimmed.split('-').collect();
        let year: i64 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let day: u32 = parts[2].parse().ok()?;

        let y = if month <= 2 { year - 1 } else { year };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = (y - era * 400) as u64;
        let m = month as u64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + (day as u64) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = (era * 146_097 + (doe as i64)) - 719_468;
        let start = days * 86_400;
        return Some(if end_of_day { start + 86_399 } else { start });
    }
    if let Ok(ts) = trimmed.parse::<i64>() {
        if ts > 0 {
            return Some(ts);
        }
    }
    None
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

fn append_filter_query_params(q: &ListBugsQuery, params: &mut Vec<String>, include_per_page: bool) {
    if let Some(fid) = q.filter_id {
        if fid > 0 {
            params.push(format!("filter_id={}", fid));
        }
    }
    if let Some(pid) = q.project_id {
        if pid > 0 {
            params.push(format!("project_id={}", pid));
        }
    }
    if let Some(mt) = q.match_type {
        if mt == FILTER_MATCH_ANY {
            params.push("match_type=1".to_string());
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
    for v in q.effective_statuses() {
        params.push(format!("status={}", v));
    }
    if let Some(hide_status) = q.hide_status {
        if hide_status > 0 {
            params.push(format!("hide_status={}", hide_status));
        }
    }
    for v in merge_positive_i32(None, &q.hide_statuses) {
        params.push(format!("hide_statuses={}", v));
    }
    for v in q.effective_priorities() {
        params.push(format!("priority={}", v));
    }
    for v in q.effective_severities() {
        params.push(format!("severity={}", v));
    }
    for v in q.effective_category_ids() {
        params.push(format!("category_id={}", v));
    }
    for v in q.effective_reporter_ids() {
        params.push(format!("reporter_id={}", v));
    }
    for v in q.effective_handler_ids() {
        params.push(format!("handler_id={}", v));
    }
    for v in q.effective_tag_ids() {
        params.push(format!("tag_id={}", v));
    }
    for v in q.effective_monitored_by_ids() {
        params.push(format!("monitored_by={}", v));
    }
    if let Some(commented_by) = q.commented_by.filter(|&v| v > 0) {
        params.push(format!("commented_by={}", commented_by));
    }
    for v in merge_positive_i32(None, &q.resolutions) {
        params.push(format!("resolution={}", v));
    }
    for v in merge_positive_i32(None, &q.reproducibilities) {
        params.push(format!("reproducibility={}", v));
    }
    if let Some(vs) = q.view_state {
        if vs == 10 || vs == 50 {
            params.push(format!("view_state={}", vs));
        }
    }
    if let Some(st) = q.sticky {
        if st == 0 || st == 1 {
            params.push(format!("sticky={}", st));
        }
    }
    for v in dedup_non_empty_strings(&q.versions) {
        params.push(format!("version={}", url_encode(&v)));
    }
    for v in q.effective_target_versions() {
        params.push(format!("target_version={}", url_encode(&v)));
    }
    for v in q.effective_fixed_in_versions() {
        params.push(format!("fixed_in_version={}", url_encode(&v)));
    }
    if let Some(ref d) = q.date_submitted_from {
        if !d.trim().is_empty() {
            params.push(format!("date_submitted_from={}", url_encode(d.trim())));
        }
    }
    if let Some(ref d) = q.date_submitted_to {
        if !d.trim().is_empty() {
            params.push(format!("date_submitted_to={}", url_encode(d.trim())));
        }
    }
    if let Some(ref d) = q.last_updated_from {
        if !d.trim().is_empty() {
            params.push(format!("last_updated_from={}", url_encode(d.trim())));
        }
    }
    if let Some(ref d) = q.last_updated_to {
        if !d.trim().is_empty() {
            params.push(format!("last_updated_to={}", url_encode(d.trim())));
        }
    }
    if let Some(rt) = q.relationship_type {
        if (-1..=6).contains(&rt) {
            params.push(format!("relationship_type={}", rt));
        }
    }
    if let Some(rbid) = q.relationship_bug_id {
        if rbid > 0 {
            params.push(format!("relationship_bug_id={}", rbid));
        }
    }
    for v in merge_positive_i64(None, &q.note_user_ids) {
        params.push(format!("note_user_id={}", v));
    }
    for (cf_id, vals) in &q.custom_fields {
        if *cf_id > 0 {
            for val in dedup_non_empty_strings(vals) {
                params.push(format!("custom_field_{}={}", cf_id, url_encode(&val)));
            }
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
    if include_per_page {
        if let Some(per_page) = q.per_page {
            if per_page > 0 {
                params.push(format!("per_page={}", clamp_per_page(Some(per_page))));
            }
        }
    }
}

pub fn build_view_all_bug_page_url(q: &ListBugsQuery, page: i64) -> String {
    let mut params = Vec::new();
    params.push(format!("page={}", page.max(1)));
    append_filter_query_params(q, &mut params, true);
    format!("/view_all_bug_page?{}", params.join("&"))
}

pub fn build_filter_permalink_url(filter: &BugFilter, filter_id: Option<i64>) -> String {
    if let Some(fid) = filter_id.filter(|&id| id > 0) {
        return format!("/view_all_bug_page?filter_id={}", fid);
    }
    let mut q = filter.to_list_query();
    q.page = None;
    let mut params = Vec::new();
    append_filter_query_params(&q, &mut params, true);
    if params.is_empty() {
        "/view_all_bug_page".to_string()
    } else {
        format!("/view_all_bug_page?{}", params.join("&"))
    }
}

fn build_export_url(base_path: &str, q: &ListBugsQuery) -> String {
    let mut params = Vec::new();
    append_filter_query_params(q, &mut params, false);
    if params.is_empty() {
        base_path.to_string()
    } else {
        format!("{}?{}", base_path, params.join("&"))
    }
}

pub fn build_csv_export_url(q: &ListBugsQuery) -> String {
    build_export_url("/csv_export", q)
}

pub fn build_print_all_bug_page_url(q: &ListBugsQuery) -> String {
    build_export_url("/print_all_bug_page", q)
}

pub fn build_excel_export_url(q: &ListBugsQuery) -> String {
    build_export_url("/excel_all_bug_page", q)
}

pub fn build_word_export_url(q: &ListBugsQuery) -> String {
    build_export_url("/word_all_bug_page", q)
}

pub fn build_issues_rss_url(q: &ListBugsQuery) -> String {
    build_issues_rss_url_for(q, 0)
}

pub fn build_issues_rss_url_for(q: &ListBugsQuery, active_project_id: i64) -> String {
    let mut url = build_export_url("/issues_rss", q);
    if active_project_id > 0 && q.project_id.is_none() {
        let sep = if url.contains('?') { '&' } else { '?' };
        url.push_str(&format!("{}project_id={}", sep, active_project_id));
    }
    url
}

pub fn sort_bugs(bugs: &mut [Bug], sort_key: Option<&str>, dir: Option<&str>) {
    let is_asc = matches!(dir, Some(d) if d.eq_ignore_ascii_case("asc"));

    let key = match sort_key {
        Some("id") => "id",
        Some("priority") => "priority",
        Some("severity") => "severity",
        Some("status") => "status",
        Some("summary") => "summary",
        Some("resolution") => "resolution",
        Some("reproducibility") => "reproducibility",
        Some("view_state") => "view_state",
        Some("date_submitted") => "date_submitted",
        Some("due_date") => "due_date",
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
            "resolution" => a.resolution.cmp(&b.resolution),
            "reproducibility" => a.reproducibility.cmp(&b.reproducibility),
            "view_state" => a.view_state.cmp(&b.view_state),
            "date_submitted" => a.date_submitted.cmp(&b.date_submitted),
            "due_date" => a.due_date.cmp(&b.due_date),
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
