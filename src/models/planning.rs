use super::bug::Bug;
use super::project::ProjectVersion;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapVersionGroup {
    pub version: String,
    pub released: bool,
    pub bugs: Vec<Bug>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub date_order: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryCounts {
    pub open: usize,
    pub resolved: usize,
    pub closed: usize,
}

impl SummaryCounts {
    pub fn record_status(&mut self, status: i32) {
        if status == 90 {
            self.closed += 1;
        } else if status >= 80 {
            self.resolved += 1;
        } else {
            self.open += 1;
        }
    }

    pub fn total(&self) -> usize {
        self.open + self.resolved + self.closed
    }
}

/// Pure domain helper that groups bugs by active project versions,
/// filtering out obsolete versions per `(project_id, version)`,
/// sorting by version `date_order DESC`, then `version ASC`, `project_id ASC`,
/// and appending remaining unknown target versions.
pub fn build_roadmap_groups(
    accessible_project_ids: &[i64],
    all_project_versions: Vec<ProjectVersion>,
    bugs: Vec<Bug>,
    project_names: &HashMap<i64, String>,
) -> Vec<RoadmapVersionGroup> {
    let accessible_set: HashSet<i64> = accessible_project_ids.iter().copied().collect();

    // Exclude versions with obsolete == 1 from the Roadmap, scoped by (project_id, version)
    let mut obsolete_versions: HashSet<(i64, String)> = HashSet::new();
    let mut active_versions = Vec::new();
    for v in all_project_versions {
        if v.obsolete == 1 {
            obsolete_versions.insert((v.project_id, v.version));
        } else {
            active_versions.push(v);
        }
    }

    // Key bugs by (project_id, target_version), excluding obsolete versions
    let mut bugs_by_proj_target: HashMap<(i64, String), Vec<Bug>> = HashMap::new();
    for b in bugs {
        if !accessible_set.contains(&b.project_id) {
            continue;
        }
        let tv = b.target_version.trim().to_string();
        if !tv.is_empty() {
            let key = (b.project_id, tv);
            if !obsolete_versions.contains(&key) {
                bugs_by_proj_target.entry(key).or_default().push(b);
            }
        }
    }

    // Sort active versions by date_order DESC, then version ASC, then project_id ASC
    active_versions.sort_by(|a, b| {
        b.date_order
            .cmp(&a.date_order)
            .then_with(|| a.version.cmp(&b.version))
            .then_with(|| a.project_id.cmp(&b.project_id))
    });

    let mut seen_keys = HashSet::new();
    let mut version_groups = Vec::new();

    for v in active_versions {
        let key = (v.project_id, v.version.clone());
        if seen_keys.insert(key.clone()) {
            let v_bugs = bugs_by_proj_target.remove(&key).unwrap_or_default();
            let project_name = project_names.get(&v.project_id).cloned();
            version_groups.push(RoadmapVersionGroup {
                version: v.version,
                released: v.released == 1,
                bugs: v_bugs,
                project_name,
                project_id: Some(v.project_id),
                date_order: v.date_order,
            });
        }
    }

    // Append remaining unknown (b.project_id, target_version) groups sorted by target_version ASC, project_id ASC
    let mut remaining_keys: Vec<(i64, String)> = bugs_by_proj_target.keys().cloned().collect();
    remaining_keys.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));

    for key in remaining_keys {
        let v_bugs = bugs_by_proj_target.remove(&key).unwrap_or_default();
        let project_name = project_names.get(&key.0).cloned();
        version_groups.push(RoadmapVersionGroup {
            version: key.1,
            released: false,
            bugs: v_bugs,
            project_name,
            project_id: Some(key.0),
            date_order: 0,
        });
    }

    version_groups
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailMessage {
    pub email_id: i64,
    pub email: String,
    pub subject: String,
    pub metadata: String,
    pub body: String,
    pub submitted: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: i64,
    pub project_id: i64,
    pub poster_id: i64,
    pub date_posted: i64,
    pub last_modified: i64,
    pub view_state: i32,
    pub announcement: i32,
    pub headline: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeTrackingEntry {
    pub bug_id: i64,
    pub bug_summary: String,
    pub user_id: i64,
    pub username: String,
    pub minutes: i32,
    pub date_submitted: i64,
    pub note: String,
    #[serde(default)]
    pub project_id: i64,
    #[serde(default)]
    pub project_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BillingIssueSummary {
    pub bug_id: i64,
    pub bug_summary: String,
    pub project_name: String,
    pub minutes: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BillingFilterOptions {
    pub from: Option<String>,
    pub to: Option<String>,
    pub include_subprojects: bool,
    pub hourly_rate: f64,
}

pub const MAX_TIME_TRACKING_MINUTES: i32 = 100_000;

pub fn clamp_time_tracking_minutes(raw: i64) -> i32 {
    raw.clamp(0, i64::from(MAX_TIME_TRACKING_MINUTES)) as i32
}

pub fn compute_billing_cost(minutes: i64, hourly_rate: f64) -> f64 {
    if hourly_rate.is_finite() && hourly_rate > 0.0 && minutes > 0 {
        let capped_rate = hourly_rate.min(1_000_000.0);
        (minutes as f64) * capped_rate / 60.0
    } else {
        0.0
    }
}

pub const MY_VIEW_CONFIG_ID: &str = "my_view_boxes";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MyViewBox {
    Assigned,
    Unassigned,
    Reported,
    Resolved,
    RecentMod,
    Monitored,
    Feedback,
    Verify,
    MyComments,
}

impl MyViewBox {
    pub fn all() -> [Self; 9] {
        [
            Self::Assigned,
            Self::Unassigned,
            Self::Reported,
            Self::Resolved,
            Self::RecentMod,
            Self::Monitored,
            Self::Feedback,
            Self::Verify,
            Self::MyComments,
        ]
    }

    pub fn key(&self) -> &'static str {
        match self {
            Self::Assigned => "assigned",
            Self::Unassigned => "unassigned",
            Self::Reported => "reported",
            Self::Resolved => "resolved",
            Self::RecentMod => "recent_mod",
            Self::Monitored => "monitored",
            Self::Feedback => "feedback",
            Self::Verify => "verify",
            Self::MyComments => "my_comments",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key.trim() {
            "assigned" => Some(Self::Assigned),
            "unassigned" => Some(Self::Unassigned),
            "reported" => Some(Self::Reported),
            "resolved" => Some(Self::Resolved),
            "recent_mod" => Some(Self::RecentMod),
            "monitored" => Some(Self::Monitored),
            "feedback" => Some(Self::Feedback),
            "verify" => Some(Self::Verify),
            "my_comments" => Some(Self::MyComments),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Assigned => "Assigned to Me (Unresolved)",
            Self::Unassigned => "Unassigned",
            Self::Reported => "Reported by Me",
            Self::Resolved => "Resolved",
            Self::RecentMod => "Recently Modified",
            Self::Monitored => "Monitored by Me",
            Self::Feedback => "Awaiting Feedback",
            Self::Verify => "Waiting for Verification",
            Self::MyComments => "Issues I Have Commented On",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Assigned => "Open or unresolved issues currently assigned to you.",
            Self::Unassigned => "Open issues not yet assigned to any developer.",
            Self::Reported => "Issues you originally reported.",
            Self::Resolved => "Issues marked as resolved across accessible projects.",
            Self::RecentMod => "Latest updated issues ordered by last modified timestamp.",
            Self::Monitored => "Issues you are explicitly monitoring.",
            Self::Feedback => "Issues in feedback status (20) waiting for reporter input.",
            Self::Verify => "Issues you reported that are now resolved and awaiting verification.",
            Self::MyComments => "Issues where you have authored at least one bugnote.",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Assigned => "fa-user",
            Self::Unassigned => "fa-inbox",
            Self::Reported => "fa-bug",
            Self::Resolved => "fa-check-square-o",
            Self::RecentMod => "fa-history",
            Self::Monitored => "fa-eye",
            Self::Feedback => "fa-commenting-o",
            Self::Verify => "fa-check-circle-o",
            Self::MyComments => "fa-comments-o",
        }
    }

    pub fn widget_id(&self) -> &'static str {
        match self {
            Self::Assigned => "my_view_assigned",
            Self::Unassigned => "my_view_unassigned",
            Self::Reported => "my_view_reported",
            Self::Resolved => "my_view_resolved",
            Self::RecentMod => "my_view_recent",
            Self::Monitored => "my_view_monitored",
            Self::Feedback => "my_view_feedback",
            Self::Verify => "my_view_verify",
            Self::MyComments => "my_view_my_comments",
        }
    }

    pub fn default_enabled(&self) -> bool {
        matches!(
            self,
            Self::Assigned
                | Self::Unassigned
                | Self::Reported
                | Self::Resolved
                | Self::RecentMod
                | Self::Monitored
        )
    }

    pub fn default_order(&self) -> i32 {
        match self {
            Self::Assigned => 1,
            Self::Unassigned => 2,
            Self::Reported => 3,
            Self::Resolved => 4,
            Self::RecentMod => 5,
            Self::Monitored => 6,
            Self::Feedback => 7,
            Self::Verify => 8,
            Self::MyComments => 9,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyViewBoxConfigEntry {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub order: i32,
}

fn default_show_timeline() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyViewConfig {
    #[serde(default)]
    pub boxes: Vec<MyViewBoxConfigEntry>,
    #[serde(default = "default_show_timeline")]
    pub show_timeline: bool,
}

impl Default for MyViewConfig {
    fn default() -> Self {
        let boxes = MyViewBox::all()
            .into_iter()
            .map(|b| MyViewBoxConfigEntry {
                key: b.key().to_string(),
                enabled: b.default_enabled(),
                order: b.default_order(),
            })
            .collect();
        Self {
            boxes,
            show_timeline: true,
        }
    }
}

impl MyViewConfig {
    pub fn normalize(self) -> Self {
        let mut by_key: HashMap<MyViewBox, MyViewBoxConfigEntry> = HashMap::new();
        for entry in self.boxes {
            if let Some(kind) = MyViewBox::from_key(&entry.key) {
                by_key.entry(kind).or_insert_with(|| MyViewBoxConfigEntry {
                    key: kind.key().to_string(),
                    enabled: entry.enabled,
                    order: entry.order.clamp(1, 99),
                });
            }
        }

        let mut normalized: Vec<(usize, MyViewBoxConfigEntry)> = MyViewBox::all()
            .into_iter()
            .enumerate()
            .map(|(idx, kind)| {
                let entry = by_key
                    .remove(&kind)
                    .unwrap_or_else(|| MyViewBoxConfigEntry {
                        key: kind.key().to_string(),
                        enabled: kind.default_enabled(),
                        order: kind.default_order(),
                    });
                (idx, entry)
            })
            .collect();

        normalized.sort_by(|a, b| a.1.order.cmp(&b.1.order).then_with(|| a.0.cmp(&b.0)));

        Self {
            boxes: normalized.into_iter().map(|(_, e)| e).collect(),
            show_timeline: self.show_timeline,
        }
    }

    pub fn enabled_boxes(&self) -> Vec<MyViewBox> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for entry in &self.boxes {
            if !entry.enabled {
                continue;
            }
            if let Some(kind) = MyViewBox::from_key(&entry.key) {
                if seen.insert(kind) {
                    out.push(kind);
                }
            }
        }
        out
    }
}

pub const SUMMARY_DATE_PARTITIONS: [i64; 9] = [1, 2, 3, 7, 30, 60, 90, 180, 365];
pub const SUMMARY_RESOLUTION_CODES: [i32; 9] = [10, 20, 30, 40, 50, 60, 70, 80, 90];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryDateBucket {
    pub days: i64,
    pub opened: usize,
    pub resolved: usize,
    pub balance: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryOpenIssue {
    pub id: i64,
    pub summary: String,
    pub days_open: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryActiveIssue {
    pub id: i64,
    pub summary: String,
    pub activity_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryUserResolutionRow {
    pub username: String,
    pub counts_by_resolution: BTreeMap<i32, usize>,
    pub total: usize,
    pub pct_fixed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReporterEffectivenessRow {
    pub username: String,
    pub total_bugs: usize,
    pub severity_score: i64,
    pub false_score: i64,
    pub effectiveness: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrendPoint {
    pub label: String,
    pub opened: usize,
    pub resolved: usize,
    pub cumulative_open: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendedSummaryData {
    pub total_bugs: usize,
    pub open_count: usize,
    pub resolved_count: usize,
    pub closed_count: usize,
    pub by_project: BTreeMap<String, SummaryCounts>,
    pub by_status: BTreeMap<i32, SummaryCounts>,
    pub by_severity: BTreeMap<i32, SummaryCounts>,
    pub by_priority: BTreeMap<i32, SummaryCounts>,
    pub by_category: BTreeMap<String, SummaryCounts>,
    pub by_resolution: BTreeMap<i32, SummaryCounts>,
    pub by_reporter: BTreeMap<String, SummaryCounts>,
    pub by_handler: BTreeMap<String, SummaryCounts>,
    pub by_date: Vec<SummaryDateBucket>,
    pub longest_open: Vec<SummaryOpenIssue>,
    pub most_active: Vec<SummaryActiveIssue>,
    pub dev_resolution: Vec<SummaryUserResolutionRow>,
    pub reporter_effectiveness: Vec<ReporterEffectivenessRow>,
    pub trend_days: i64,
    pub trend_points: Vec<TrendPoint>,
}

fn severity_multiplier(severity: i32) -> i64 {
    match severity {
        10 => 1,  // feature
        20 => 2,  // trivial
        30 => 3,  // text
        40 => 2,  // tweak
        50 => 5,  // minor
        60 => 8,  // major
        70 => 8,  // crash
        80 => 10, // block
        _ => 1,
    }
}

fn resolution_error_multiplier(resolution: i32) -> i64 {
    match resolution {
        40 => 2, // unable to reproduce
        50 => 1, // not fixable
        60 => 3, // duplicate
        70 => 5, // not a bug / no change required
        80 => 1, // suspended
        90 => 1, // won't fix
        _ => 0,
    }
}

fn format_utc_ymd(ts: i64) -> String {
    if ts <= 0 {
        return "1970-01-01".to_string();
    }
    let days = ts.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", year, m, d)
}

pub fn clamp_trend_days(days: i64) -> i64 {
    match days {
        7 => 7,
        90 => 90,
        365 => 365,
        _ => 30,
    }
}

pub fn build_issues_trend(
    bugs: &[Bug],
    resolved_at_map: &HashMap<i64, i64>,
    now_ts: i64,
    days: i64,
) -> Vec<TrendPoint> {
    let clamped_days = clamp_trend_days(days);
    let buckets = match clamped_days {
        7 => 7i64,
        30 => 10i64,
        _ => 12i64,
    };
    let end_ts = now_ts.max(86_400);
    let total_span = clamped_days * 86_400;
    let start_ts = end_ts - total_span;

    let mut out = Vec::with_capacity(buckets as usize);
    for idx in 0..buckets {
        let b_start = start_ts + (total_span * idx) / buckets;
        let b_end = start_ts + (total_span * (idx + 1)) / buckets;

        let mut opened = 0usize;
        let mut resolved = 0usize;
        let mut cumulative_open = 0usize;

        for b in bugs {
            let in_open = if idx == 0 {
                b.date_submitted >= b_start && b.date_submitted <= b_end
            } else {
                b.date_submitted > b_start && b.date_submitted <= b_end
            };
            if b.date_submitted > 0 && in_open {
                opened += 1;
            }

            let eff_resolved_ts = resolved_at_map
                .get(&b.id)
                .copied()
                .filter(|&t| t > 0)
                .unwrap_or(b.last_updated);
            if b.status >= 80 {
                let in_res = if idx == 0 {
                    eff_resolved_ts >= b_start && eff_resolved_ts <= b_end
                } else {
                    eff_resolved_ts > b_start && eff_resolved_ts <= b_end
                };
                if eff_resolved_ts > 0 && in_res {
                    resolved += 1;
                }
            }

            if b.date_submitted <= b_end && (b.status < 80 || eff_resolved_ts > b_end) {
                cumulative_open += 1;
            }
        }

        out.push(TrendPoint {
            label: format_utc_ymd(b_end),
            opened,
            resolved,
            cumulative_open,
        });
    }

    out
}

#[allow(clippy::too_many_arguments)]
pub fn compute_extended_summary(
    bugs: &[Bug],
    now_ts: i64,
    projects_map: &HashMap<i64, String>,
    categories_map: &HashMap<i64, String>,
    users_map: &HashMap<i64, String>,
    history_counts: &HashMap<i64, usize>,
    resolved_at_map: &HashMap<i64, i64>,
    trend_days: i64,
) -> ExtendedSummaryData {
    let mut by_project: BTreeMap<String, SummaryCounts> = BTreeMap::new();
    let mut by_status: BTreeMap<i32, SummaryCounts> = BTreeMap::new();
    let mut by_severity: BTreeMap<i32, SummaryCounts> = BTreeMap::new();
    let mut by_priority: BTreeMap<i32, SummaryCounts> = BTreeMap::new();
    let mut by_category: BTreeMap<String, SummaryCounts> = BTreeMap::new();
    let mut by_resolution: BTreeMap<i32, SummaryCounts> = BTreeMap::new();
    let mut by_reporter: BTreeMap<String, SummaryCounts> = BTreeMap::new();
    let mut by_handler: BTreeMap<String, SummaryCounts> = BTreeMap::new();

    let mut open_count = 0usize;
    let mut resolved_count = 0usize;
    let mut closed_count = 0usize;

    let mut dev_res_acc: HashMap<String, BTreeMap<i32, usize>> = HashMap::new();
    let mut rep_eff_acc: HashMap<String, (usize, i64, i64)> = HashMap::new();

    for b in bugs {
        let proj_name = projects_map
            .get(&b.project_id)
            .cloned()
            .unwrap_or_else(|| format!("Project #{}", b.project_id));
        by_project
            .entry(proj_name)
            .or_default()
            .record_status(b.status);

        by_status
            .entry(b.status)
            .or_default()
            .record_status(b.status);
        by_severity
            .entry(b.severity)
            .or_default()
            .record_status(b.status);
        by_priority
            .entry(b.priority)
            .or_default()
            .record_status(b.status);
        by_resolution
            .entry(b.resolution)
            .or_default()
            .record_status(b.status);

        let cat_name = categories_map
            .get(&b.category_id)
            .cloned()
            .unwrap_or_else(|| "(none)".to_string());
        by_category
            .entry(cat_name)
            .or_default()
            .record_status(b.status);

        let rep = users_map
            .get(&b.reporter_id)
            .cloned()
            .unwrap_or_else(|| format!("user#{}", b.reporter_id));
        by_reporter
            .entry(rep.clone())
            .or_default()
            .record_status(b.status);

        let sev_mult = severity_multiplier(b.severity);
        let res_err_mult = resolution_error_multiplier(b.resolution);
        let rep_entry = rep_eff_acc.entry(rep).or_insert((0, 0, 0));
        rep_entry.0 += 1;
        rep_entry.1 += sev_mult;
        rep_entry.2 += sev_mult * res_err_mult;

        if b.handler_id > 0 {
            let hnd = users_map
                .get(&b.handler_id)
                .cloned()
                .unwrap_or_else(|| format!("user#{}", b.handler_id));
            by_handler
                .entry(hnd.clone())
                .or_default()
                .record_status(b.status);
            *dev_res_acc
                .entry(hnd)
                .or_default()
                .entry(b.resolution)
                .or_default() += 1;
        }

        if b.status == 90 {
            closed_count += 1;
        } else if b.status >= 80 {
            resolved_count += 1;
        } else {
            open_count += 1;
        }
    }

    let by_date: Vec<SummaryDateBucket> = SUMMARY_DATE_PARTITIONS
        .iter()
        .map(|&days| {
            let cutoff = now_ts - days * 86_400;
            let mut opened = 0usize;
            let mut resolved = 0usize;
            for b in bugs {
                if b.date_submitted > 0 && b.date_submitted >= cutoff && b.date_submitted <= now_ts
                {
                    opened += 1;
                }
                if b.status >= 80 {
                    let eff_resolved = resolved_at_map
                        .get(&b.id)
                        .copied()
                        .filter(|&t| t > 0)
                        .unwrap_or(b.last_updated);
                    if eff_resolved > 0 && eff_resolved >= cutoff && eff_resolved <= now_ts {
                        resolved += 1;
                    }
                }
            }
            SummaryDateBucket {
                days,
                opened,
                resolved,
                balance: (opened as i64) - (resolved as i64),
            }
        })
        .collect();

    let mut open_bugs: Vec<&Bug> = bugs.iter().filter(|b| b.status < 80).collect();
    open_bugs.sort_by(|a, b| {
        a.date_submitted
            .cmp(&b.date_submitted)
            .then_with(|| b.priority.cmp(&a.priority))
            .then_with(|| a.id.cmp(&b.id))
    });
    let longest_open: Vec<SummaryOpenIssue> = open_bugs
        .into_iter()
        .take(10)
        .map(|b| SummaryOpenIssue {
            id: b.id,
            summary: b.summary.clone(),
            days_open: if b.date_submitted > 0 {
                (now_ts - b.date_submitted).max(0) / 86_400
            } else {
                0
            },
        })
        .collect();

    let mut active_candidates: Vec<(&Bug, usize)> = bugs
        .iter()
        .filter(|b| b.status < 80)
        .filter_map(|b| {
            let cnt = history_counts.get(&b.id).copied().unwrap_or(0);
            (cnt > 0).then_some((b, cnt))
        })
        .collect();
    active_candidates.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.0.last_updated.cmp(&a.0.last_updated))
            .then_with(|| b.0.id.cmp(&a.0.id))
    });
    let most_active: Vec<SummaryActiveIssue> = active_candidates
        .into_iter()
        .take(10)
        .map(|(b, cnt)| SummaryActiveIssue {
            id: b.id,
            summary: b.summary.clone(),
            activity_count: cnt,
        })
        .collect();

    let mut dev_resolution: Vec<SummaryUserResolutionRow> = dev_res_acc
        .into_iter()
        .map(|(username, counts_by_resolution)| {
            let total: usize = counts_by_resolution.values().sum();
            let fixed = counts_by_resolution.get(&20).copied().unwrap_or(0);
            let pct_fixed = (fixed * 100).checked_div(total).unwrap_or(0);
            SummaryUserResolutionRow {
                username,
                counts_by_resolution,
                total,
                pct_fixed,
            }
        })
        .collect();
    dev_resolution.sort_by(|a, b| {
        b.total
            .cmp(&a.total)
            .then_with(|| a.username.cmp(&b.username))
    });
    dev_resolution.truncate(20);

    let mut reporter_effectiveness: Vec<ReporterEffectivenessRow> = rep_eff_acc
        .into_iter()
        .map(
            |(username, (total_bugs, severity_score, false_score))| ReporterEffectivenessRow {
                username,
                total_bugs,
                severity_score,
                false_score,
                effectiveness: severity_score - false_score,
            },
        )
        .collect();
    reporter_effectiveness.sort_by(|a, b| {
        b.total_bugs
            .cmp(&a.total_bugs)
            .then_with(|| a.username.cmp(&b.username))
    });
    reporter_effectiveness.truncate(20);

    let clamped_days = clamp_trend_days(trend_days);
    let trend_points = build_issues_trend(bugs, resolved_at_map, now_ts, clamped_days);

    ExtendedSummaryData {
        total_bugs: bugs.len(),
        open_count,
        resolved_count,
        closed_count,
        by_project,
        by_status,
        by_severity,
        by_priority,
        by_category,
        by_resolution,
        by_reporter,
        by_handler,
        by_date,
        longest_open,
        most_active,
        dev_resolution,
        reporter_effectiveness,
        trend_days: clamped_days,
        trend_points,
    }
}
