use crate::db::Db;
use crate::mcp::protocol::{CallToolResult, ToolDefinition};
use crate::models::{AccessLevel, BugFilter, BugUpdate, User};
use serde_json::{json, Value};

pub fn all_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "whoami".to_string(),
            description: "Get details of the currently authenticated user whose identity and permissions this session operates under.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "list_projects".to_string(),
            description: "List all projects accessible to the current user with their ID, name, status, and view state.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "get_project_metadata".to_string(),
            description: "Get categories, versions, and metadata available for a project.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "integer",
                        "description": "The unique ID of the project"
                    }
                },
                "required": ["project_id"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "search_issues".to_string(),
            description: "Search and filter issues by project, status, handler, reporter, or keyword across accessible projects.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "integer", "description": "Filter by project ID" },
                    "status": { "type": "integer", "description": "Filter by status (e.g. 10: new, 50: assigned, 80: resolved, 90: closed)" },
                    "priority": { "type": "integer", "description": "Filter by priority (10: none, 20: low, 30: normal, 40: high, 50: urgent, 60: immediate)" },
                    "severity": { "type": "integer", "description": "Filter by severity (10: feature, 20: trivial, 30: text, 40: tweak, 50: minor, 60: major, 70: crash, 80: block)" },
                    "handler_id": { "type": "integer", "description": "Filter by assigned user ID" },
                    "reporter_id": { "type": "integer", "description": "Filter by reporting user ID" },
                    "search": { "type": "string", "description": "Keyword search across summary and description" },
                    "limit": { "type": "integer", "description": "Max results to return (default: 25, max: 100)" },
                    "page": { "type": "integer", "description": "Page number starting at 1 (default: 1)" }
                },
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "get_issue".to_string(),
            description: "Get complete details of a specific issue, including description, steps to reproduce, additional info, bugnotes, and tags.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "issue_id": {
                        "type": "integer",
                        "description": "The unique ID of the issue"
                    }
                },
                "required": ["issue_id"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "create_issue".to_string(),
            description: "Report a new issue in a project. Automatically records the authenticated user as the reporter.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "integer", "description": "Target project ID" },
                    "category_id": { "type": "integer", "description": "Category ID within the project" },
                    "summary": { "type": "string", "description": "Short summary of the bug/task" },
                    "description": { "type": "string", "description": "Detailed description of the issue" },
                    "priority": { "type": "integer", "description": "Priority (10: none, 20: low, 30: normal, 40: high, 50: urgent, 60: immediate)" },
                    "severity": { "type": "integer", "description": "Severity (10: feature, 20: trivial, 30: text, 40: tweak, 50: minor, 60: major, 70: crash, 80: block)" },
                    "reproducibility": { "type": "integer", "description": "Reproducibility (10: always, 30: sometimes, 50: random, 70: not tried, 90: unable to reproduce, 100: N/A)" },
                    "steps_to_reproduce": { "type": "string", "description": "Step-by-step reproduction instructions" },
                    "additional_information": { "type": "string", "description": "Additional context or system info" },
                    "view_state": { "type": "integer", "description": "View state (10: public, 50: private)" }
                },
                "required": ["project_id", "category_id", "summary", "description"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "update_issue".to_string(),
            description: "Update an existing issue's status, resolution, handler, priority, severity, or summary.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer", "description": "ID of the issue to update" },
                    "status": { "type": "integer", "description": "New status (10: new, 20: feedback, 30: acknowledged, 40: confirmed, 50: assigned, 80: resolved, 90: closed)" },
                    "resolution": { "type": "integer", "description": "Resolution code (10: open, 20: fixed, 30: reopened, 40: unable to duplicate, 50: not fixable, 60: duplicate, 70: not a bug, 80: suspended, 90: won't fix)" },
                    "handler_id": { "type": "integer", "description": "User ID to assign issue to (requires Developer 55+)" },
                    "priority": { "type": "integer", "description": "Updated priority" },
                    "severity": { "type": "integer", "description": "Updated severity" },
                    "summary": { "type": "string", "description": "Updated summary" }
                },
                "required": ["issue_id"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "add_issue_note".to_string(),
            description: "Add a comment/bugnote to an existing issue. Automatically records the authenticated user as the author.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "issue_id": { "type": "integer", "description": "ID of the issue" },
                    "text": { "type": "string", "description": "Content of the note" },
                    "view_state": { "type": "integer", "description": "Note visibility (10: public, 50: private - private requires Developer 55+)" },
                    "time_tracking": { "type": "integer", "description": "Time spent in minutes (optional)" }
                },
                "required": ["issue_id", "text"],
                "additionalProperties": false
            }),
        },
    ]
}

pub fn dispatch_tool_call(db: &Db, user: &User, name: &str, args: Value) -> CallToolResult {
    match name {
        "whoami" => {
            let access_level_name = AccessLevel::from_i32(user.access_level)
                .map(|a| a.as_str().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let data = json!({
                "user_id": user.id,
                "username": user.username,
                "real_name": user.realname,
                "email": user.email,
                "access_level": user.access_level,
                "access_level_name": access_level_name
            });
            CallToolResult::ok_json(&data)
        }

        "list_projects" => {
            let projects = match db.list_visible_projects(user) {
                Ok(p) => p,
                Err(e) => return CallToolResult::error(format!("Failed to list projects: {}", e)),
            };
            let out: Vec<Value> = projects
                .into_iter()
                .map(|(p, depth)| {
                    json!({
                        "id": p.id,
                        "name": p.name,
                        "status": p.status,
                        "enabled": p.enabled == 1,
                        "view_state": p.view_state,
                        "description": p.description,
                        "depth": depth
                    })
                })
                .collect();
            CallToolResult::ok_json(&out)
        }

        "get_project_metadata" => {
            let project_id = match args.get("project_id").and_then(|v| v.as_i64()) {
                Some(id) if id > 0 => id,
                _ => return CallToolResult::error("Project ID must be greater than 0"),
            };

            let eff = db.effective_access_level(user, project_id).unwrap_or(0);
            if eff < 10 {
                return CallToolResult::error(format!(
                    "Permission Denied: Viewer (10) or higher access required on project {}",
                    project_id
                ));
            }

            let categories = db
                .list_categories_for_project(project_id)
                .unwrap_or_default();
            let versions = db.list_versions(project_id).unwrap_or_default();

            let data = json!({
                "project_id": project_id,
                "categories": categories.into_iter().map(|c| json!({ "id": c.id, "name": c.name })).collect::<Vec<_>>(),
                "versions": versions.into_iter().map(|v| json!({ "id": v.id, "version": v.version, "released": v.released == 1 })).collect::<Vec<_>>()
            });
            CallToolResult::ok_json(&data)
        }

        "search_issues" => {
            let project_id = args.get("project_id").and_then(|v| v.as_i64());
            if let Some(pid) = project_id {
                let eff = db.effective_access_level(user, pid).unwrap_or(0);
                if eff < 10 {
                    return CallToolResult::error(format!(
                        "Permission Denied: Viewer (10) or higher access required on project {}",
                        pid
                    ));
                }
            }

            let limit = args
                .get("limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(25)
                .min(100) as usize;
            let page = args.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let offset = (page.saturating_sub(1)) * limit;

            let filter = BugFilter {
                project_id,
                search: args
                    .get("search")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                status: args
                    .get("status")
                    .and_then(|v| v.as_i64())
                    .map(|s| s as i32),
                hide_status: None,
                priority: args
                    .get("priority")
                    .and_then(|v| v.as_i64())
                    .map(|p| p as i32),
                severity: args
                    .get("severity")
                    .and_then(|v| v.as_i64())
                    .map(|s| s as i32),
                category_id: None,
                reporter_id: args.get("reporter_id").and_then(|v| v.as_i64()),
                handler_id: args.get("handler_id").and_then(|v| v.as_i64()),
                tag_id: None,
                monitored_by: None,
            };

            let bugs = match db.filter_bugs(&filter, user) {
                Ok(b) => b,
                Err(e) => return CallToolResult::error(format!("Failed to search issues: {}", e)),
            };

            let total = bugs.len();
            let paged_bugs: Vec<Value> = bugs
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(|b| {
                    json!({
                        "id": b.id,
                        "project_id": b.project_id,
                        "summary": b.summary,
                        "status": b.status,
                        "priority": b.priority,
                        "severity": b.severity,
                        "reporter_id": b.reporter_id,
                        "handler_id": b.handler_id,
                        "date_submitted": b.date_submitted,
                        "last_updated": b.last_updated,
                        "view_state": b.view_state
                    })
                })
                .collect();

            let data = json!({
                "total": total,
                "page": page,
                "limit": limit,
                "issues": paged_bugs
            });
            CallToolResult::ok_json(&data)
        }

        "get_issue" => {
            let issue_id = match args.get("issue_id").and_then(|v| v.as_i64()) {
                Some(id) => id,
                None => return CallToolResult::error("Missing required parameter: issue_id"),
            };

            let bug = match db.get_bug(issue_id) {
                Ok(Some(b)) => b,
                Ok(None) => return CallToolResult::error(format!("Issue #{} not found", issue_id)),
                Err(e) => return CallToolResult::error(format!("Database error: {}", e)),
            };

            let eff = db.effective_access_level(user, bug.project_id).unwrap_or(0);
            if eff < 10 || (bug.view_state == 50 && eff < 55 && bug.reporter_id != user.id) {
                return CallToolResult::error(format!(
                    "Access Denied to issue #{}. Minimum Developer (55) required for private issues.",
                    issue_id
                ));
            }

            let full = match db.get_bug_full(issue_id) {
                Ok(Some(f)) => f,
                _ => return CallToolResult::error("Failed to load full issue details"),
            };

            let notes = db.list_bugnotes(issue_id, eff).unwrap_or_default();
            let notes_json: Vec<Value> = notes
                .into_iter()
                .map(|n| {
                    json!({
                        "id": n.id,
                        "reporter_id": n.reporter_id,
                        "text": n.note,
                        "view_state": n.view_state,
                        "date_submitted": n.date_submitted,
                        "time_tracking": n.time_tracking
                    })
                })
                .collect();

            let data = json!({
                "id": full.bug.id,
                "project_id": full.bug.project_id,
                "category_id": full.bug.category_id,
                "summary": full.bug.summary,
                "description": full.description,
                "steps_to_reproduce": full.steps_to_reproduce,
                "additional_information": full.additional_information,
                "status": full.bug.status,
                "resolution": full.bug.resolution,
                "priority": full.bug.priority,
                "severity": full.bug.severity,
                "reproducibility": full.bug.reproducibility,
                "reporter_id": full.bug.reporter_id,
                "handler_id": full.bug.handler_id,
                "view_state": full.bug.view_state,
                "date_submitted": full.bug.date_submitted,
                "last_updated": full.bug.last_updated,
                "notes": notes_json
            });
            CallToolResult::ok_json(&data)
        }

        "create_issue" => {
            let project_id = match args.get("project_id").and_then(|v| v.as_i64()) {
                Some(id) if id > 0 => id,
                _ => return CallToolResult::error("Project ID must be greater than 0"),
            };
            let category_id = match args.get("category_id").and_then(|v| v.as_i64()) {
                Some(id) if id > 0 => id,
                _ => return CallToolResult::error("Category ID must be greater than 0"),
            };
            let summary = match args.get("summary").and_then(|v| v.as_str()) {
                Some(s) if !s.trim().is_empty() => s.trim(),
                _ => return CallToolResult::error("Missing or empty required parameter: summary"),
            };
            let description = match args.get("description").and_then(|v| v.as_str()) {
                Some(d) if !d.trim().is_empty() => d.trim(),
                _ => {
                    return CallToolResult::error(
                        "Missing or empty required parameter: description",
                    )
                }
            };

            let eff = db.effective_access_level(user, project_id).unwrap_or(0);
            if eff < 25 {
                return CallToolResult::error(format!(
                    "Permission Denied: Reporter (25) or higher access required to create issues on project {} (your level: {})",
                    project_id, eff
                ));
            }

            let categories = db.list_effective_categories(project_id).unwrap_or_default();
            if !categories.iter().any(|c| c.id == category_id) {
                return CallToolResult::error(format!(
                    "Category #{} not found or does not belong to project #{}",
                    category_id, project_id
                ));
            }

            let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(30) as i32;
            let severity = args.get("severity").and_then(|v| v.as_i64()).unwrap_or(50) as i32;
            let reproducibility = args
                .get("reproducibility")
                .and_then(|v| v.as_i64())
                .unwrap_or(10) as i32;
            let steps = args
                .get("steps_to_reproduce")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let add_info = args
                .get("additional_information")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let view_state = args
                .get("view_state")
                .and_then(|v| v.as_i64())
                .unwrap_or(10) as i32;

            match db.create_bug(
                project_id,
                user.id,
                category_id,
                summary,
                description,
                steps,
                add_info,
                priority,
                severity,
                reproducibility,
                "",
                "",
                "",
                "",
                view_state,
            ) {
                Ok(bug_id) => {
                    let data = json!({
                        "issue_id": bug_id,
                        "project_id": project_id,
                        "reporter_id": user.id,
                        "summary": summary,
                        "status": 10
                    });
                    CallToolResult::ok_json(&data)
                }
                Err(e) => CallToolResult::error(format!("Failed to create issue: {}", e)),
            }
        }

        "update_issue" => {
            let issue_id = match args.get("issue_id").and_then(|v| v.as_i64()) {
                Some(id) => id,
                None => return CallToolResult::error("Missing required parameter: issue_id"),
            };

            let bug = match db.get_bug(issue_id) {
                Ok(Some(b)) => b,
                Ok(None) => return CallToolResult::error(format!("Issue #{} not found", issue_id)),
                Err(e) => return CallToolResult::error(format!("Database error: {}", e)),
            };

            let eff = db.effective_access_level(user, bug.project_id).unwrap_or(0);
            if bug.view_state == 50 && eff < 55 && bug.reporter_id != user.id {
                return CallToolResult::error(format!(
                    "Access Denied to private issue #{}. Minimum Developer (55) required.",
                    issue_id
                ));
            }

            let can_edit = eff >= 40 || (eff >= 25 && bug.reporter_id == user.id);
            if !can_edit {
                return CallToolResult::error(format!(
                    "Permission Denied: Updater (40) or Reporter on own issue required to update issue #{} (your level: {})",
                    issue_id, eff
                ));
            }

            let handler_id_opt = args.get("handler_id").and_then(|v| v.as_i64());
            if handler_id_opt.is_some() && eff < 55 {
                return CallToolResult::error(format!(
                    "Permission Denied: Developer (55) or higher required to assign handler on issue #{} (your level: {})",
                    issue_id, eff
                ));
            }

            // Validate status transition permission BEFORE making any updates to prevent partial writes
            let status_opt = args
                .get("status")
                .and_then(|v| v.as_i64())
                .map(|s| s as i32);

            if let Some(st) = status_opt {
                let required_level = if st == 90 {
                    70 // Close requires Manager (70+)
                } else if st == 80 || bug.status >= 80 {
                    55 // Resolve or reopen requires Developer (55+)
                } else {
                    40 // Status transitions require Updater (40+)
                };

                if eff < required_level {
                    return CallToolResult::error(format!(
                        "Permission Denied: Access level {} required for status transition to {} (your level: {})",
                        required_level, st, eff
                    ));
                }
            }

            let mut update = BugUpdate::default();
            let mut has_field_updates = false;

            if let Some(h) = handler_id_opt {
                update.handler_id = Some(h);
                has_field_updates = true;
            }
            if let Some(p) = args.get("priority").and_then(|v| v.as_i64()) {
                update.priority = Some(p as i32);
                has_field_updates = true;
            }
            if let Some(s) = args.get("severity").and_then(|v| v.as_i64()) {
                update.severity = Some(s as i32);
                has_field_updates = true;
            }
            if let Some(sm) = args.get("summary").and_then(|v| v.as_str()) {
                update.summary = Some(sm.to_string());
                has_field_updates = true;
            }

            if has_field_updates {
                if let Err(e) = db.update_bug_fields(issue_id, user.id, update) {
                    return CallToolResult::error(format!("Failed to update issue fields: {}", e));
                }
            }

            if let Some(st) = status_opt {
                let res = args
                    .get("resolution")
                    .and_then(|v| v.as_i64())
                    .map(|r| r as i32)
                    .unwrap_or(bug.resolution);
                if let Err(e) = db.change_bug_status(issue_id, user.id, st, res, None) {
                    return CallToolResult::error(format!("Failed to update issue status: {}", e));
                }
            }

            let data = json!({
                "issue_id": issue_id,
                "updated": true
            });
            CallToolResult::ok_json(&data)
        }

        "add_issue_note" => {
            let issue_id = match args.get("issue_id").and_then(|v| v.as_i64()) {
                Some(id) => id,
                None => return CallToolResult::error("Missing required parameter: issue_id"),
            };
            let text = match args.get("text").and_then(|v| v.as_str()) {
                Some(t) if !t.trim().is_empty() => t.trim(),
                _ => return CallToolResult::error("Missing or empty required parameter: text"),
            };

            let bug = match db.get_bug(issue_id) {
                Ok(Some(b)) => b,
                Ok(None) => return CallToolResult::error(format!("Issue #{} not found", issue_id)),
                Err(e) => return CallToolResult::error(format!("Database error: {}", e)),
            };

            let eff = db.effective_access_level(user, bug.project_id).unwrap_or(0);
            if eff < 25 || (bug.view_state == 50 && eff < 55 && bug.reporter_id != user.id) {
                return CallToolResult::error(format!(
                    "Permission Denied: Reporter (25) or higher required to add bugnotes to issue #{}",
                    issue_id
                ));
            }

            let view_state = args
                .get("view_state")
                .and_then(|v| v.as_i64())
                .unwrap_or(10) as i32;
            if view_state == 50 && eff < 55 {
                return CallToolResult::error(
                    "Permission Denied: Developer (55) or higher required to create private notes",
                );
            }
            let raw_time = args
                .get("time_tracking")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let time_tracking = if raw_time > 0 {
                if eff < 55 {
                    return CallToolResult::error(
                        "Permission Denied: Developer (55) or higher required to record time tracking",
                    );
                }
                raw_time.max(0) as i32
            } else {
                0
            };

            match db.add_bugnote_with_time(issue_id, user.id, text, view_state, time_tracking) {
                Ok(note_id) => {
                    let data = json!({
                        "note_id": note_id,
                        "issue_id": issue_id,
                        "reporter_id": user.id
                    });
                    CallToolResult::ok_json(&data)
                }
                Err(e) => CallToolResult::error(format!("Failed to add note: {}", e)),
            }
        }

        _ => CallToolResult::error(format!("Unknown tool: {}", name)),
    }
}
