use super::bug::Bug;
use super::project::ProjectVersion;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoadmapVersionGroup {
    pub version: String,
    pub released: bool,
    pub bugs: Vec<Bug>,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub project_id: Option<i64>,
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
}
