use super::*;
use std::collections::HashMap;

#[test]
fn test_access_level_conversion_and_strings() {
    assert_eq!(AccessLevel::from_i32(10), Some(AccessLevel::Viewer));
    assert_eq!(AccessLevel::from_i32(25), Some(AccessLevel::Reporter));
    assert_eq!(AccessLevel::from_i32(40), Some(AccessLevel::Updater));
    assert_eq!(AccessLevel::from_i32(55), Some(AccessLevel::Developer));
    assert_eq!(AccessLevel::from_i32(70), Some(AccessLevel::Manager));
    assert_eq!(AccessLevel::from_i32(90), Some(AccessLevel::Administrator));
    assert_eq!(AccessLevel::from_i32(100), Some(AccessLevel::Nobody));
    assert_eq!(AccessLevel::from_i32(999), None);

    assert_eq!(AccessLevel::Viewer.as_str(), "viewer");
    assert_eq!(AccessLevel::Reporter.as_str(), "reporter");
    assert_eq!(AccessLevel::Updater.as_str(), "updater");
    assert_eq!(AccessLevel::Developer.as_str(), "developer");
    assert_eq!(AccessLevel::Manager.as_str(), "manager");
    assert_eq!(AccessLevel::Administrator.as_str(), "administrator");
    assert_eq!(AccessLevel::Nobody.as_str(), "nobody");

    assert!(AccessLevel::Administrator > AccessLevel::Manager);
    assert!(AccessLevel::Manager > AccessLevel::Developer);
}

#[test]
fn test_project_status_conversion_and_strings() {
    assert_eq!(
        ProjectStatus::from_i32(10),
        Some(ProjectStatus::Development)
    );
    assert_eq!(ProjectStatus::from_i32(30), Some(ProjectStatus::Release));
    assert_eq!(ProjectStatus::from_i32(50), Some(ProjectStatus::Stable));
    assert_eq!(ProjectStatus::from_i32(70), Some(ProjectStatus::Obsolete));
    assert_eq!(ProjectStatus::from_i32(0), None);

    assert_eq!(ProjectStatus::Development.as_str(), "development");
    assert_eq!(ProjectStatus::Release.as_str(), "release");
    assert_eq!(ProjectStatus::Stable.as_str(), "stable");
    assert_eq!(ProjectStatus::Obsolete.as_str(), "obsolete");
}

#[test]
fn test_view_state_conversion_and_strings() {
    assert_eq!(ViewState::from_i32(10), Some(ViewState::Public));
    assert_eq!(ViewState::from_i32(50), Some(ViewState::Private));
    assert_eq!(ViewState::from_i32(100), None);

    assert_eq!(ViewState::Public.as_str(), "public");
    assert_eq!(ViewState::Private.as_str(), "private");
}

#[test]
fn test_bug_status_conversion_and_strings() {
    assert_eq!(BugStatus::from_i32(10), Some(BugStatus::New));
    assert_eq!(BugStatus::from_i32(20), Some(BugStatus::Feedback));
    assert_eq!(BugStatus::from_i32(30), Some(BugStatus::Acknowledged));
    assert_eq!(BugStatus::from_i32(40), Some(BugStatus::Confirmed));
    assert_eq!(BugStatus::from_i32(50), Some(BugStatus::Assigned));
    assert_eq!(BugStatus::from_i32(80), Some(BugStatus::Resolved));
    assert_eq!(BugStatus::from_i32(90), Some(BugStatus::Closed));
    assert_eq!(BugStatus::from_i32(999), None);

    assert_eq!(BugStatus::New.as_str(), "new");
    assert_eq!(BugStatus::Feedback.as_str(), "feedback");
    assert_eq!(BugStatus::Acknowledged.as_str(), "acknowledged");
    assert_eq!(BugStatus::Confirmed.as_str(), "confirmed");
    assert_eq!(BugStatus::Assigned.as_str(), "assigned");
    assert_eq!(BugStatus::Resolved.as_str(), "resolved");
    assert_eq!(BugStatus::Closed.as_str(), "closed");

    assert!(BugStatus::Closed > BugStatus::New);
}

#[test]
fn test_bug_priority_conversion_and_strings() {
    assert_eq!(BugPriority::from_i32(10), Some(BugPriority::None));
    assert_eq!(BugPriority::from_i32(20), Some(BugPriority::Low));
    assert_eq!(BugPriority::from_i32(30), Some(BugPriority::Normal));
    assert_eq!(BugPriority::from_i32(40), Some(BugPriority::High));
    assert_eq!(BugPriority::from_i32(50), Some(BugPriority::Urgent));
    assert_eq!(BugPriority::from_i32(60), Some(BugPriority::Immediate));
    assert_eq!(BugPriority::from_i32(0), None);

    assert_eq!(BugPriority::None.as_str(), "none");
    assert_eq!(BugPriority::Low.as_str(), "low");
    assert_eq!(BugPriority::Normal.as_str(), "normal");
    assert_eq!(BugPriority::High.as_str(), "high");
    assert_eq!(BugPriority::Urgent.as_str(), "urgent");
    assert_eq!(BugPriority::Immediate.as_str(), "immediate");
}

#[test]
fn test_bug_severity_conversion_and_strings() {
    assert_eq!(BugSeverity::from_i32(10), Some(BugSeverity::Feature));
    assert_eq!(BugSeverity::from_i32(20), Some(BugSeverity::Trivial));
    assert_eq!(BugSeverity::from_i32(30), Some(BugSeverity::Text));
    assert_eq!(BugSeverity::from_i32(40), Some(BugSeverity::Tweak));
    assert_eq!(BugSeverity::from_i32(50), Some(BugSeverity::Minor));
    assert_eq!(BugSeverity::from_i32(60), Some(BugSeverity::Major));
    assert_eq!(BugSeverity::from_i32(70), Some(BugSeverity::Crash));
    assert_eq!(BugSeverity::from_i32(80), Some(BugSeverity::Block));
    assert_eq!(BugSeverity::from_i32(999), None);

    assert_eq!(BugSeverity::Feature.as_str(), "feature");
    assert_eq!(BugSeverity::Trivial.as_str(), "trivial");
    assert_eq!(BugSeverity::Text.as_str(), "text");
    assert_eq!(BugSeverity::Tweak.as_str(), "tweak");
    assert_eq!(BugSeverity::Minor.as_str(), "minor");
    assert_eq!(BugSeverity::Major.as_str(), "major");
    assert_eq!(BugSeverity::Crash.as_str(), "crash");
    assert_eq!(BugSeverity::Block.as_str(), "block");
}

#[test]
fn test_bug_reproducibility_conversion_and_strings() {
    assert_eq!(
        BugReproducibility::from_i32(10),
        Some(BugReproducibility::Always)
    );
    assert_eq!(
        BugReproducibility::from_i32(30),
        Some(BugReproducibility::Sometimes)
    );
    assert_eq!(
        BugReproducibility::from_i32(50),
        Some(BugReproducibility::Random)
    );
    assert_eq!(
        BugReproducibility::from_i32(70),
        Some(BugReproducibility::HaveNotTried)
    );
    assert_eq!(
        BugReproducibility::from_i32(90),
        Some(BugReproducibility::UnableToReproduce)
    );
    assert_eq!(
        BugReproducibility::from_i32(100),
        Some(BugReproducibility::NA)
    );
    assert_eq!(BugReproducibility::from_i32(0), None);

    assert_eq!(BugReproducibility::Always.as_str(), "always");
    assert_eq!(BugReproducibility::Sometimes.as_str(), "sometimes");
    assert_eq!(BugReproducibility::Random.as_str(), "random");
    assert_eq!(BugReproducibility::HaveNotTried.as_str(), "have not tried");
    assert_eq!(
        BugReproducibility::UnableToReproduce.as_str(),
        "unable to reproduce"
    );
    assert_eq!(BugReproducibility::NA.as_str(), "N/A");
}

#[test]
fn test_bug_resolution_conversion_and_strings() {
    assert_eq!(BugResolution::from_i32(10), Some(BugResolution::Open));
    assert_eq!(BugResolution::from_i32(20), Some(BugResolution::Fixed));
    assert_eq!(BugResolution::from_i32(30), Some(BugResolution::Reopened));
    assert_eq!(
        BugResolution::from_i32(40),
        Some(BugResolution::UnableToReproduce)
    );
    assert_eq!(BugResolution::from_i32(50), Some(BugResolution::NotFixable));
    assert_eq!(BugResolution::from_i32(60), Some(BugResolution::Duplicate));
    assert_eq!(BugResolution::from_i32(70), Some(BugResolution::NotABug));
    assert_eq!(BugResolution::from_i32(80), Some(BugResolution::Suspended));
    assert_eq!(BugResolution::from_i32(90), Some(BugResolution::WontFix));
    assert_eq!(BugResolution::from_i32(-1), None);

    assert_eq!(BugResolution::Open.as_str(), "open");
    assert_eq!(BugResolution::Fixed.as_str(), "fixed");
    assert_eq!(BugResolution::Reopened.as_str(), "reopened");
    assert_eq!(
        BugResolution::UnableToReproduce.as_str(),
        "unable to reproduce"
    );
    assert_eq!(BugResolution::NotFixable.as_str(), "not fixable");
    assert_eq!(BugResolution::Duplicate.as_str(), "duplicate");
    assert_eq!(BugResolution::NotABug.as_str(), "not a bug");
    assert_eq!(BugResolution::Suspended.as_str(), "suspended");
    assert_eq!(BugResolution::WontFix.as_str(), "won't fix");
}

#[test]
fn test_relationship_type_conversion_and_strings() {
    assert_eq!(
        RelationshipType::from_i32(0),
        Some(RelationshipType::RelatedTo)
    );
    assert_eq!(
        RelationshipType::from_i32(1),
        Some(RelationshipType::DuplicateOf)
    );
    assert_eq!(
        RelationshipType::from_i32(2),
        Some(RelationshipType::HasDuplicate)
    );
    assert_eq!(
        RelationshipType::from_i32(3),
        Some(RelationshipType::Blocks)
    );
    assert_eq!(
        RelationshipType::from_i32(4),
        Some(RelationshipType::BlockedBy)
    );
    assert_eq!(
        RelationshipType::from_i32(5),
        Some(RelationshipType::ParentOf)
    );
    assert_eq!(
        RelationshipType::from_i32(6),
        Some(RelationshipType::ChildOf)
    );
    assert_eq!(RelationshipType::from_i32(7), None);

    assert_eq!(RelationshipType::RelatedTo.as_str(), "related to");
    assert_eq!(RelationshipType::DuplicateOf.as_str(), "duplicate of");
    assert_eq!(RelationshipType::HasDuplicate.as_str(), "has duplicate");
    assert_eq!(RelationshipType::Blocks.as_str(), "blocks");
    assert_eq!(RelationshipType::BlockedBy.as_str(), "blocked by");
    assert_eq!(RelationshipType::ParentOf.as_str(), "parent of");
    assert_eq!(RelationshipType::ChildOf.as_str(), "child of");
}

#[test]
fn test_custom_field_type_conversion_and_strings() {
    assert_eq!(CustomFieldType::from_i32(0), Some(CustomFieldType::String));
    assert_eq!(CustomFieldType::from_i32(1), Some(CustomFieldType::Numeric));
    assert_eq!(CustomFieldType::from_i32(2), Some(CustomFieldType::Float));
    assert_eq!(CustomFieldType::from_i32(3), Some(CustomFieldType::Enum));
    assert_eq!(CustomFieldType::from_i32(4), Some(CustomFieldType::Email));
    assert_eq!(
        CustomFieldType::from_i32(5),
        Some(CustomFieldType::Checkbox)
    );
    assert_eq!(CustomFieldType::from_i32(6), Some(CustomFieldType::List));
    assert_eq!(
        CustomFieldType::from_i32(7),
        Some(CustomFieldType::Multilist)
    );
    assert_eq!(CustomFieldType::from_i32(8), Some(CustomFieldType::Date));
    assert_eq!(CustomFieldType::from_i32(9), Some(CustomFieldType::Radio));
    assert_eq!(
        CustomFieldType::from_i32(10),
        Some(CustomFieldType::Textarea)
    );
    assert_eq!(CustomFieldType::from_i32(11), None);

    assert_eq!(CustomFieldType::String.as_str(), "string");
    assert_eq!(CustomFieldType::Numeric.as_str(), "numeric");
    assert_eq!(CustomFieldType::Float.as_str(), "float");
    assert_eq!(CustomFieldType::Enum.as_str(), "enumeration");
    assert_eq!(CustomFieldType::Email.as_str(), "email");
    assert_eq!(CustomFieldType::Checkbox.as_str(), "checkbox");
    assert_eq!(CustomFieldType::List.as_str(), "list");
    assert_eq!(CustomFieldType::Multilist.as_str(), "multiselection list");
    assert_eq!(CustomFieldType::Date.as_str(), "date");
    assert_eq!(CustomFieldType::Radio.as_str(), "radio");
    assert_eq!(CustomFieldType::Textarea.as_str(), "textarea");
}

#[test]
fn test_jira_status_mapping_serialization() {
    let mapping = JiraStatusMapping {
        jira_status: "In Progress".to_string(),
        mantis_status: 50,
    };
    let json = serde_json::to_string(&mapping).unwrap();
    let deserialized: JiraStatusMapping = serde_json::from_str(&json).unwrap();
    assert_eq!(mapping, deserialized);
}

#[test]
fn test_bug_enums_as_str() {
    assert_eq!(BugStatus::New.as_str(), "new");
    assert_eq!(BugStatus::Feedback.as_str(), "feedback");
    assert_eq!(BugStatus::Acknowledged.as_str(), "acknowledged");
    assert_eq!(BugStatus::Confirmed.as_str(), "confirmed");
    assert_eq!(BugStatus::Assigned.as_str(), "assigned");
    assert_eq!(BugStatus::Resolved.as_str(), "resolved");
    assert_eq!(BugStatus::Closed.as_str(), "closed");

    assert_eq!(BugResolution::Open.as_str(), "open");
    assert_eq!(BugResolution::Fixed.as_str(), "fixed");
    assert_eq!(BugPriority::Normal.as_str(), "normal");
    assert_eq!(BugSeverity::Minor.as_str(), "minor");
    assert_eq!(BugReproducibility::Always.as_str(), "always");
}

#[test]
fn test_bug_filter_json_backward_compatibility() {
    // Existing JSON format stored in mantis_filters_table without sort, dir, per_page
    let legacy_json = r#"{"project_id":1,"search":"deadlock","status":10,"hide_status":80,"priority":40,"severity":60,"category_id":2,"reporter_id":3,"handler_id":-1,"tag_id":4,"monitored_by":5}"#;
    let bf: BugFilter = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(bf.project_id, Some(1));
    assert_eq!(bf.search.as_deref(), Some("deadlock"));
    assert_eq!(bf.status, Some(10));
    assert_eq!(bf.hide_status, Some(80));
    assert_eq!(bf.priority, Some(40));
    assert_eq!(bf.severity, Some(60));
    assert_eq!(bf.category_id, Some(2));
    assert_eq!(bf.reporter_id, Some(3));
    assert_eq!(bf.handler_id, Some(-1));
    assert_eq!(bf.tag_id, Some(4));
    assert_eq!(bf.monitored_by, Some(5));
    // Defaults for newly added fields
    assert_eq!(bf.sort, None);
    assert_eq!(bf.dir, None);
    assert_eq!(bf.per_page, None);

    // New JSON format with sort, dir, per_page
    let new_json =
        r#"{"project_id":1,"search":"crash","sort":"priority","dir":"asc","per_page":50}"#;
    let bf2: BugFilter = serde_json::from_str(new_json).unwrap();
    assert_eq!(bf2.sort.as_deref(), Some("priority"));
    assert_eq!(bf2.dir.as_deref(), Some("asc"));
    assert_eq!(bf2.per_page, Some(50));

    // Roundtrip serialization
    let serialized = serde_json::to_string(&bf2).unwrap();
    let deserialized: BugFilter = serde_json::from_str(&serialized).unwrap();
    assert_eq!(bf2, deserialized);
}

#[test]
fn test_deserialize_lenient_per_page() {
    // Non-numeric or invalid per_page strings must deserialize cleanly to None without error
    let json_abc: ListBugsQuery = serde_json::from_str(r#"{"per_page": "abc"}"#).unwrap();
    assert_eq!(json_abc.per_page, None);
    assert_eq!(clamp_per_page(json_abc.per_page), 25);

    let json_neg: ListBugsQuery = serde_json::from_str(r#"{"per_page": "-1"}"#).unwrap();
    assert_eq!(json_neg.per_page, None);
    assert_eq!(clamp_per_page(json_neg.per_page), 25);

    let json_empty: ListBugsQuery = serde_json::from_str(r#"{"per_page": ""}"#).unwrap();
    assert_eq!(json_empty.per_page, None);
    assert_eq!(clamp_per_page(json_empty.per_page), 25);

    let json_int: ListBugsQuery = serde_json::from_str(r#"{"per_page": 50}"#).unwrap();
    assert_eq!(json_int.per_page, Some(50));
    assert_eq!(clamp_per_page(json_int.per_page), 50);

    let json_str_valid: ListBugsQuery = serde_json::from_str(r#"{"per_page": "100"}"#).unwrap();
    assert_eq!(json_str_valid.per_page, Some(100));
    assert_eq!(clamp_per_page(json_str_valid.per_page), 100);

    let json_missing: ListBugsQuery = serde_json::from_str(r#"{"page": 2}"#).unwrap();
    assert_eq!(json_missing.per_page, None);
    assert_eq!(clamp_per_page(json_missing.per_page), 25);

    let json_null: ListBugsQuery = serde_json::from_str(r#"{"per_page": null}"#).unwrap();
    assert_eq!(json_null.per_page, None);
    assert_eq!(clamp_per_page(json_null.per_page), 25);
}

#[test]
fn test_build_csv_export_url() {
    let q = ListBugsQuery {
        page: Some(3),
        filter_id: Some(10),
        q: Some("buffer overflow".to_string()),
        search: None,
        status: Some(50),
        hide_status: Some(80),
        priority: Some(60),
        severity: Some(70),
        category_id: Some(2),
        reporter_id: Some(5),
        handler_id: Some(8),
        tag_id: Some(3),
        monitored_by: Some(4),
        msg: Some("flash_msg".to_string()),
        err: Some("flash_err".to_string()),
        sort: Some("summary".to_string()),
        dir: Some("asc".to_string()),
        per_page: Some(50),
    };

    let url = build_csv_export_url(&q);

    // Starts with /csv_export?
    assert!(url.starts_with("/csv_export?"));
    // Must NOT contain page, per_page, or flash messages
    assert!(!url.contains("page="));
    assert!(!url.contains("per_page="));
    assert!(!url.contains("msg="));
    assert!(!url.contains("err="));
    // Preserves active filters and sorting
    assert!(url.contains("filter_id=10"));
    assert!(url.contains("q=buffer+overflow"));
    assert!(url.contains("status=50"));
    assert!(url.contains("hide_status=80"));
    assert!(url.contains("priority=60"));
    assert!(url.contains("severity=70"));
    assert!(url.contains("category_id=2"));
    assert!(url.contains("reporter_id=5"));
    assert!(url.contains("handler_id=8"));
    assert!(url.contains("tag_id=3"));
    assert!(url.contains("monitored_by=4"));
    assert!(url.contains("sort=summary"));
    assert!(url.contains("dir=asc"));

    // Empty query produces clean /csv_export
    let empty_q = ListBugsQuery::default();
    assert_eq!(build_csv_export_url(&empty_q), "/csv_export");
}

#[test]
fn test_bug_update_defaults_and_fields() {
    let update = BugUpdate::default();
    assert!(update.summary.is_none());
    assert!(update.os.is_none());
    assert!(update.os_build.is_none());
    assert!(update.platform.is_none());
    assert!(update.build.is_none());
    assert!(update.due_date.is_none());
    assert!(update.sticky.is_none());

    let populated = BugUpdate {
        summary: Some("Test".to_string()),
        os: Some("Linux".to_string()),
        os_build: Some("6.1".to_string()),
        platform: Some("x86_64".to_string()),
        build: Some("b100".to_string()),
        due_date: Some(1700000000),
        sticky: Some(1),
        ..Default::default()
    };
    let json = serde_json::to_string(&populated).unwrap();
    let deserialized: BugUpdate = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.summary, Some("Test".to_string()));
    assert_eq!(deserialized.os, Some("Linux".to_string()));
    assert_eq!(deserialized.os_build, Some("6.1".to_string()));
    assert_eq!(deserialized.platform, Some("x86_64".to_string()));
    assert_eq!(deserialized.build, Some("b100".to_string()));
    assert_eq!(deserialized.due_date, Some(1700000000));
    assert_eq!(deserialized.sticky, Some(1));
}

#[test]
fn test_resolve_possible_values_tokens_and_static() {
    let versions = vec![
        ProjectVersion {
            id: 1,
            project_id: 1,
            version: "1.0".to_string(),
            description: "".to_string(),
            released: 1,
            obsolete: 0,
            date_order: 1,
        },
        ProjectVersion {
            id: 2,
            project_id: 1,
            version: "2.0".to_string(),
            description: "".to_string(),
            released: 0,
            obsolete: 0,
            date_order: 2,
        },
        ProjectVersion {
            id: 3,
            project_id: 1,
            version: "0.9".to_string(),
            description: "".to_string(),
            released: 1,
            obsolete: 1,
            date_order: 0,
        },
    ];

    let categories = vec![
        Category {
            id: 10,
            project_id: 1,
            user_id: 0,
            name: "Backend".to_string(),
            status: 0,
        },
        Category {
            id: 11,
            project_id: 1,
            user_id: 0,
            name: "Frontend".to_string(),
            status: 0,
        },
    ];

    // =versions: non-obsolete versions (1.0, 2.0)
    let res = resolve_possible_values("=versions", &versions, &categories);
    assert_eq!(res, vec!["1.0", "2.0"]);

    // =released_versions: released & non-obsolete (1.0)
    let res = resolve_possible_values("=released_versions", &versions, &categories);
    assert_eq!(res, vec!["1.0"]);

    // =future_versions: unreleased & non-obsolete (2.0)
    let res = resolve_possible_values("=future_versions", &versions, &categories);
    assert_eq!(res, vec!["2.0"]);

    // =categories: project categories (Backend, Frontend)
    let res = resolve_possible_values("=categories", &versions, &categories);
    assert_eq!(res, vec!["Backend", "Frontend"]);

    // Static pipe-delimited values
    let res = resolve_possible_values("Red|Green|Blue", &versions, &categories);
    assert_eq!(res, vec!["Red", "Green", "Blue"]);

    // Static with whitespace
    let res = resolve_possible_values("  High | Medium | Low  ", &versions, &categories);
    assert_eq!(res, vec!["High", "Medium", "Low"]);

    // Empty
    let res = resolve_possible_values("", &versions, &categories);
    assert!(res.is_empty());
}

#[test]
fn test_ymd_validation_and_timestamp_conversion() {
    // Valid YMD
    assert!(is_valid_ymd("2024-02-29")); // leap year
    assert!(is_valid_ymd("2020-02-29")); // leap year
    assert!(is_valid_ymd("2000-02-29")); // 400 year leap
    assert!(is_valid_ymd("2023-12-31"));
    assert!(is_valid_ymd("1970-01-01"));

    // Invalid YMD
    assert!(!is_valid_ymd("2023-02-29")); // not a leap year
    assert!(!is_valid_ymd("1900-02-29")); // 100 year non-leap
    assert!(!is_valid_ymd("2023-04-31")); // April has 30 days
    assert!(!is_valid_ymd("2023-13-01")); // invalid month
    assert!(!is_valid_ymd("2023-00-10")); // invalid month
    assert!(!is_valid_ymd("2023-05-00")); // invalid day
    assert!(!is_valid_ymd("2023-05-32")); // invalid day
    assert!(!is_valid_ymd("abcd-ef-gh")); // non-numeric
    assert!(!is_valid_ymd("2023/05/15")); // wrong separator
    assert!(!is_valid_ymd("2023-5-15")); // non 2-digit month
    assert!(!is_valid_ymd(""));

    // Unix timestamp conversion
    assert_eq!(timestamp_to_ymd(0), Some("1970-01-01".to_string()));
    assert_eq!(timestamp_to_ymd(1709251200), Some("2024-03-01".to_string()));
}

#[test]
fn test_validate_custom_field_value_all_types() {
    fn make_cf(
        field_type: i32,
        possible_values: &str,
        length_min: i32,
        length_max: i32,
    ) -> CustomField {
        CustomField {
            id: 1,
            name: "Test Field".to_string(),
            field_type,
            possible_values: possible_values.to_string(),
            default_value: "".to_string(),
            valid_regexp: "".to_string(),
            access_level_r: 10,
            access_level_rw: 25,
            length_min,
            length_max,
            require_report: 0,
            require_update: 0,
            display_report: 1,
            display_update: 1,
            require_resolved: 0,
            display_resolved: 0,
            display_closed: 0,
            require_closed: 0,
            filter_by: 1,
        }
    }

    // Empty value when not required is valid and clears field
    let cf_str = make_cf(0, "", 3, 10);
    assert!(validate_custom_field_value(&cf_str, "", &[], false).is_ok());
    // Empty value when required must fail
    assert!(validate_custom_field_value(&cf_str, "", &[], true).is_err());

    // 0: String length check
    assert!(validate_custom_field_value(&cf_str, "ab", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_str, "abc", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_str, "abcdefghij", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_str, "abcdefghijk", &[], false).is_err());

    // 1: Numeric (i64)
    let cf_num = make_cf(1, "", 0, 0);
    assert!(validate_custom_field_value(&cf_num, "12345", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_num, "-99", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_num, "12.34", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_num, "abc", &[], false).is_err());

    // 2: Float (f64)
    let cf_flt = make_cf(2, "", 0, 0);
    assert!(validate_custom_field_value(&cf_flt, "12.34", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_flt, "-0.5", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_flt, "100", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_flt, "NaN", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_flt, "inf", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_flt, "abc", &[], false).is_err());

    // 3: Enum & 6: List
    let opts: Vec<String> = vec!["Alpha".to_string(), "Beta".to_string(), "Gamma".to_string()];
    let cf_enum = make_cf(3, "Alpha|Beta|Gamma", 0, 0);
    assert!(validate_custom_field_value(&cf_enum, "Alpha", &opts, false).is_ok());
    assert!(validate_custom_field_value(&cf_enum, "Delta", &opts, false).is_err());

    let cf_list = make_cf(6, "Alpha|Beta|Gamma", 0, 0);
    assert!(validate_custom_field_value(&cf_list, "Beta", &opts, false).is_ok());
    assert!(validate_custom_field_value(&cf_list, "Delta", &opts, false).is_err());

    // 4: Email
    let cf_email = make_cf(4, "", 0, 0);
    assert!(validate_custom_field_value(&cf_email, "user@example.com", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_email, "name+tag@sub.domain.co", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_email, "plainaddress", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_email, "@missingusername.com", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_email, "user@domain", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_email, "user@ domain.com", &[], false).is_err());

    // 5: Checkbox & 7: Multilist
    let cf_check = make_cf(5, "Alpha|Beta|Gamma", 0, 0);
    assert!(validate_custom_field_value(&cf_check, "Alpha", &opts, false).is_ok());
    assert!(validate_custom_field_value(&cf_check, "Alpha|Beta", &opts, false).is_ok());
    assert!(validate_custom_field_value(&cf_check, "Alpha|Delta", &opts, false).is_err());

    let cf_multi = make_cf(7, "Alpha|Beta|Gamma", 0, 0);
    assert!(validate_custom_field_value(&cf_multi, "Beta|Gamma", &opts, false).is_ok());
    assert!(validate_custom_field_value(&cf_multi, "Unknown", &opts, false).is_err());

    // Choice fields with empty options list reject non-empty values
    assert!(validate_custom_field_value(&cf_enum, "Alpha", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_list, "Beta", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_multi, "Beta", &[], false).is_err());
    // Checkbox with empty options accepts "1" (boolean toggle) but rejects other values
    let cf_bool_check = make_cf(5, "", 0, 0);
    assert!(validate_custom_field_value(&cf_bool_check, "1", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_bool_check, "invalid", &[], false).is_err());

    // 8: Date
    let cf_date = make_cf(8, "", 0, 0);
    assert!(validate_custom_field_value(&cf_date, "2024-02-29", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_date, "1709251200", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_date, "2023-02-29", &[], false).is_err());
    assert!(validate_custom_field_value(&cf_date, "invalid-date", &[], false).is_err());

    // 9: Radio
    let cf_radio = make_cf(9, "Alpha|Beta|Gamma", 0, 0);
    assert!(validate_custom_field_value(&cf_radio, "Gamma", &opts, false).is_ok());
    assert!(validate_custom_field_value(&cf_radio, "Zeta", &opts, false).is_err());

    // 10: Textarea
    let cf_txt = make_cf(10, "", 5, 20);
    assert!(validate_custom_field_value(&cf_txt, "Hello world", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf_txt, "Hi", &[], false).is_err());
    assert!(validate_custom_field_value(
        &cf_txt,
        "This is way too long for a textarea with max 20",
        &[],
        false
    )
    .is_err());
}

#[test]
fn test_custom_field_valid_regexp_enforcement() {
    let mut cf = CustomField {
        id: 1,
        name: "IssueCode".to_string(),
        field_type: 0, // String
        possible_values: "".to_string(),
        default_value: "".to_string(),
        valid_regexp: r"^[A-Z]{3}-\d{4}$".to_string(),
        access_level_r: 10,
        access_level_rw: 25,
        length_min: 0,
        length_max: 0,
        require_report: 0,
        require_update: 0,
        display_report: 1,
        display_update: 1,
        require_resolved: 0,
        display_resolved: 0,
        display_closed: 0,
        require_closed: 0,
        filter_by: 1,
    };

    // 1. Matching value passes
    assert!(validate_custom_field_value(&cf, "ABC-1234", &[], false).is_ok());

    // 2. Non-matching value fails with clear message
    let err = validate_custom_field_value(&cf, "abc-1234", &[], false).unwrap_err();
    assert_eq!(
        err,
        "Custom field 'IssueCode' does not match required pattern"
    );

    let err2 = validate_custom_field_value(&cf, "ABC-12", &[], false).unwrap_err();
    assert_eq!(
        err2,
        "Custom field 'IssueCode' does not match required pattern"
    );

    // 3. Empty value on non-required field succeeds without regex evaluation
    assert!(validate_custom_field_value(&cf, "", &[], false).is_ok());
    assert!(validate_custom_field_value(&cf, "   ", &[], false).is_ok());

    // 4. Empty value on required field fails with required error
    cf.require_report = 1;
    let req_err = validate_custom_field_value(&cf, "", &[], true).unwrap_err();
    assert_eq!(req_err, "Custom field 'IssueCode' is required");

    // 5. Invalid pattern in legacy row produces informative error without crashing
    cf.valid_regexp = "[unclosed".to_string();
    let pat_err = validate_custom_field_value(&cf, "ABC-1234", &[], false).unwrap_err();
    assert!(pat_err.contains("has invalid pattern"));
}

#[test]
fn test_compile_custom_field_regex_safety_and_errors() {
    // Valid patterns
    assert!(compile_custom_field_regex(r"^\d+$").is_ok());
    assert!(
        compile_custom_field_regex(r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$").is_ok()
    );

    // Invalid syntax
    let syn_err = compile_custom_field_regex(r"(?P<invalid").unwrap_err();
    assert!(!syn_err.is_empty());

    // Pathological / oversized pattern exceeding 1 MiB size limit
    let huge_pattern = "(?:[a-zA-Z0-9]{500}){1000}";
    let size_err = compile_custom_field_regex(huge_pattern).unwrap_err();
    assert!(
        size_err.to_lowercase().contains("limit") || size_err.to_lowercase().contains("size"),
        "Expected size limit error, got: {}",
        size_err
    );
}

#[test]
fn test_validate_regex_pattern_helper() {
    // Empty or whitespace-only returns Ok(())
    assert!(validate_regex_pattern("").is_ok());
    assert!(validate_regex_pattern("   \t\n  ").is_ok());

    // Valid pattern returns Ok(())
    assert!(validate_regex_pattern(r"^[0-9]{3}-[A-Z]+$").is_ok());

    // Invalid pattern returns Err
    let err = validate_regex_pattern(r"[unclosed").unwrap_err();
    assert!(!err.is_empty());
}

#[test]
fn test_build_roadmap_groups_pure_domain_logic() {
    let mut project_names = HashMap::new();
    project_names.insert(1, "Project Alpha".to_string());
    project_names.insert(2, "Project Beta".to_string());

    let versions = vec![
        ProjectVersion {
            id: 1,
            project_id: 1,
            version: "v1.0".to_string(),
            description: "".to_string(),
            released: 1,
            obsolete: 0,
            date_order: 100,
        },
        ProjectVersion {
            id: 2,
            project_id: 1,
            version: "v2.0".to_string(),
            description: "".to_string(),
            released: 0,
            obsolete: 1, // Obsolete for Project 1
            date_order: 200,
        },
        ProjectVersion {
            id: 3,
            project_id: 2,
            version: "v2.0".to_string(),
            description: "".to_string(),
            released: 0,
            obsolete: 0, // Active for Project 2
            date_order: 150,
        },
    ];

    let make_bug = |id: i64, project_id: i64, target_version: &str| Bug {
        id,
        project_id,
        reporter_id: 1,
        handler_id: 0,
        duplicate_id: 0,
        priority: 30,
        severity: 50,
        reproducibility: 10,
        status: 50,
        resolution: 10,
        os: "".to_string(),
        os_build: "".to_string(),
        platform: "".to_string(),
        version: "".to_string(),
        fixed_in_version: "".to_string(),
        build: "".to_string(),
        view_state: 10,
        summary: format!("Bug {}", id),
        sticky: 0,
        target_version: target_version.to_string(),
        category_id: 1,
        date_submitted: 1000,
        due_date: 0,
        last_updated: 1000,
    };

    let bugs = vec![
        make_bug(101, 1, "v1.0"),
        make_bug(102, 1, "v2.0"), // Excluded: v2.0 is obsolete in project 1
        make_bug(103, 2, "v2.0"), // Included: v2.0 is active in project 2
        make_bug(104, 2, "v9.9"), // Unknown version for project 2: appended
        make_bug(105, 999, "v1.0"), // Inaccessible project 999: excluded
    ];

    let groups = build_roadmap_groups(&[1, 2], versions, bugs, &project_names);

    assert_eq!(groups.len(), 3);

    // Group 1: Project 2 v2.0 (date_order 150)
    assert_eq!(groups[0].version, "v2.0");
    assert_eq!(groups[0].project_id, Some(2));
    assert_eq!(groups[0].project_name, Some("Project Beta".to_string()));
    assert_eq!(groups[0].bugs.len(), 1);
    assert_eq!(groups[0].bugs[0].id, 103);

    // Group 2: Project 1 v1.0 (date_order 100)
    assert_eq!(groups[1].version, "v1.0");
    assert_eq!(groups[1].project_id, Some(1));
    assert_eq!(groups[1].project_name, Some("Project Alpha".to_string()));
    assert!(groups[1].released);
    assert_eq!(groups[1].bugs.len(), 1);
    assert_eq!(groups[1].bugs[0].id, 101);

    // Group 3: Project 2 v9.9 (unknown appended)
    assert_eq!(groups[2].version, "v9.9");
    assert_eq!(groups[2].project_id, Some(2));
    assert_eq!(groups[2].bugs.len(), 1);
    assert_eq!(groups[2].bugs[0].id, 104);
}
