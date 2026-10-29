use super::*;
use crate::models::Bug;

#[test]
fn test_labels() {
    assert_eq!(status_label(10), "new");
    assert_eq!(status_label(90), "closed");
    assert_eq!(status_label(999), "unknown");

    assert_eq!(priority_label(30), "normal");
    assert_eq!(priority_label(50), "urgent");
    assert_eq!(priority_label(60), "immediate");
    assert_eq!(priority_label(999), "unknown");

    assert_eq!(severity_label(50), "minor");
    assert_eq!(severity_label(80), "block");
    assert_eq!(severity_label(999), "unknown");

    assert_eq!(reproducibility_label(10), "always");
    assert_eq!(reproducibility_label(70), "have not tried");
    assert_eq!(reproducibility_label(999), "unknown");

    assert_eq!(resolution_label(10), "open");
    assert_eq!(resolution_label(20), "fixed");
    assert_eq!(resolution_label(999), "unknown");

    assert_eq!(relationship_label(0), "related to");
    assert_eq!(relationship_label(1), "duplicate of");
    assert_eq!(relationship_label(999), "related to");
}

#[test]
fn test_render_my_view_box_empty() {
    let out = render_my_view_box("Unassigned", "/view_all_bug_page", &[]);
    assert!(out.contains("Unassigned"));
    assert!(out.contains("No issues found."));
    assert!(out.contains(r#"href="/view_all_bug_page""#));
}

#[test]
fn test_render_my_view_box_with_issues() {
    let sample_bug = Bug {
        id: 7,
        project_id: 1,
        reporter_id: 1,
        handler_id: 2,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 10,
        resolution: 10,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "1.0".to_string(),
        fixed_in_version: "".to_string(),
        build: "".to_string(),
        view_state: 10,
        summary: "Crash on startup".to_string(),
        sticky: 0,
        target_version: "".to_string(),
        category_id: 1,
        date_submitted: 1700000000,
        due_date: 0,
        last_updated: 1700000000,
    };

    let out = render_my_view_box("Recent", "/view_all_bug_page", &[sample_bug]);
    assert!(out.contains("Recent"));
    assert!(out.contains("0000007"));
    assert!(out.contains("Crash on startup"));
}
