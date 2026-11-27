use super::*;

#[test]
fn test_bootstrap_creates_tables_and_default_seed() {
    let db = Db::open_in_memory().unwrap();
    let admin = db
        .get_user_by_username("administrator")
        .unwrap()
        .expect("default admin");
    assert_eq!(admin.access_level, 90);
    assert_eq!(admin.enabled, 1);

    let cats = db.list_categories_for_project(0).unwrap();
    assert_eq!(cats.len(), 1);
    assert_eq!(cats[0].name, "General");
    assert_eq!(cats[0].project_id, 0);

    db.set_config("window_title", 0, 0, "Rust MantisBT")
        .unwrap();
    assert_eq!(
        db.get_config("window_title", 0, 0).unwrap().as_deref(),
        Some("Rust MantisBT")
    );
}

#[test]
fn test_user_lookup_helpers() {
    let db = Db::open_in_memory().unwrap();
    let admin_by_name = db.get_user_by_username("administrator").unwrap().unwrap();
    let admin_by_id = db.get_user_by_id(admin_by_name.id).unwrap().unwrap();
    assert_eq!(admin_by_name.id, admin_by_id.id);

    let admin_by_cookie = db
        .get_user_by_cookie(&admin_by_name.cookie_string)
        .unwrap()
        .unwrap();
    assert_eq!(admin_by_name.id, admin_by_cookie.id);

    assert!(db.get_user_by_username("nonexistent").unwrap().is_none());
    assert!(db.get_user_by_id(9999).unwrap().is_none());
    assert!(db.get_user_by_cookie("invalid_cookie").unwrap().is_none());
}

#[test]
fn test_config_hierarchical_fallback() {
    let db = Db::open_in_memory().unwrap();

    // 1. Global config
    db.set_config("theme", 0, 0, "default").unwrap();
    assert_eq!(
        db.get_config("theme", 0, 0).unwrap().as_deref(),
        Some("default")
    );
    assert_eq!(
        db.get_config("theme", 10, 20).unwrap().as_deref(),
        Some("default")
    );

    // 2. Project config override
    db.set_config("theme", 10, 0, "project_blue").unwrap();
    assert_eq!(
        db.get_config("theme", 10, 20).unwrap().as_deref(),
        Some("project_blue")
    );
    assert_eq!(
        db.get_config("theme", 0, 0).unwrap().as_deref(),
        Some("default")
    );

    // 3. User config override
    db.set_config("theme", 10, 20, "user_dark").unwrap();
    assert_eq!(
        db.get_config("theme", 10, 20).unwrap().as_deref(),
        Some("user_dark")
    );

    // 4. Global user config override (0, user_id)
    db.set_config("language", 0, 20, "french").unwrap();
    assert_eq!(
        db.get_config("language", 10, 20).unwrap().as_deref(),
        Some("french")
    );
    assert_eq!(db.get_config("language", 10, 30).unwrap().as_deref(), None);
}

#[test]
fn test_category_inheritance() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn();

    // Create Project 1 (inherit_global = 1)
    conn.execute(
            "INSERT INTO mantis_project_table (id, name, inherit_global) VALUES (1, 'Project Alpha', 1)",
            [],
        ).unwrap();
    // Add project-specific category for Project 1
    conn.execute(
            "INSERT INTO mantis_category_table (project_id, user_id, name) VALUES (1, 0, 'Alpha Features')",
            [],
        ).unwrap();

    // Project 1 categories should include 'General' (global) and 'Alpha Features'
    drop(conn);
    let cats1 = db.list_categories_for_project(1).unwrap();
    let cat_names1: Vec<&str> = cats1.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(cat_names1, vec!["Alpha Features", "General"]);

    // Create Project 2 (child of Project 1, inherit_parent = 1)
    let conn = db.conn();
    conn.execute(
            "INSERT INTO mantis_project_table (id, name, inherit_global) VALUES (2, 'Project Alpha Sub', 0)",
            [],
        ).unwrap();
    conn.execute(
            "INSERT INTO mantis_project_hierarchy_table (child_id, parent_id, inherit_parent) VALUES (2, 1, 1)",
            [],
        ).unwrap();
    conn.execute(
            "INSERT INTO mantis_category_table (project_id, user_id, name) VALUES (2, 0, 'Sub Category')",
            [],
        ).unwrap();

    drop(conn);
    let cats2 = db.list_categories_for_project(2).unwrap();
    let cat_names2: Vec<&str> = cats2.iter().map(|c| c.name.as_str()).collect();
    // Since project 2 has inherit_parent = 1, it should inherit Project 1's categories
    assert!(cat_names2.contains(&"Sub Category"));
    assert!(cat_names2.contains(&"Alpha Features"));
}

#[test]
fn test_db_open_file_based() {
    let tmp_dir = std::env::temp_dir();
    let db_path = tmp_dir.join(format!("mantis_test_{}.db", rand::random::<u64>()));
    let db_str = db_path.to_str().unwrap();

    {
        let db = Db::open(db_str).unwrap();
        let admin = db
            .get_user_by_username("administrator")
            .unwrap()
            .expect("admin exists");
        assert_eq!(admin.access_level, 90);
    }

    // Reopen existing db
    {
        let db = Db::open(db_str).unwrap();
        let admin = db
            .get_user_by_username("administrator")
            .unwrap()
            .expect("admin exists");
        assert_eq!(admin.access_level, 90);
    }

    let _ = std::fs::remove_file(db_path);
}

#[test]
fn test_effective_access_and_private_projects() {
    let db = Db::open_in_memory().unwrap();
    let pub_id = db
        .create_project("PublicProj", 10, 10, "desc", true)
        .unwrap();
    let priv_id = db
        .create_project("PrivateProj", 10, 50, "desc", true)
        .unwrap();

    let rep_id = db
        .create_user("alice", "Alice", "a@x.com", "pw", 25, true, false)
        .unwrap();
    let mgr_id = db
        .create_user("bob", "Bob", "b@x.com", "pw", 70, true, false)
        .unwrap();
    let alice = db.get_user_by_id(rep_id).unwrap().unwrap();
    let bob = db.get_user_by_id(mgr_id).unwrap().unwrap();

    assert_eq!(db.effective_access_level(&alice, pub_id).unwrap(), 25);
    assert_eq!(db.effective_access_level(&alice, priv_id).unwrap(), 0);
    assert_eq!(db.effective_access_level(&bob, priv_id).unwrap(), 70);

    // Grant Alice Developer (55) on PrivateProj
    db.set_project_user(priv_id, alice.id, 55).unwrap();
    assert_eq!(db.effective_access_level(&alice, priv_id).unwrap(), 55);

    // Disabled project returns 0 even with explicit override
    db.update_project(priv_id, "PrivateProj", 10, false, 50, "desc", true)
        .unwrap();
    assert_eq!(db.effective_access_level(&alice, priv_id).unwrap(), 0);
    assert_eq!(db.effective_access_level(&bob, priv_id).unwrap(), 0);
}

#[test]
fn test_hierarchy_cycle_prevention_and_category_inheritance() {
    let db = Db::open_in_memory().unwrap();
    let p1 = db.create_project("Parent", 10, 10, "", true).unwrap();
    let p2 = db.create_project("Child", 10, 10, "", false).unwrap();

    db.add_subproject(p1, p2, true).unwrap();
    // Self-parent and reverse cycle must fail
    assert!(db.add_subproject(p1, p1, true).is_err());
    assert!(db.add_subproject(p2, p1, true).is_err());

    db.add_category(p1, "Backend", 0).unwrap();
    db.add_category(p2, "UI", 0).unwrap();

    // Child inherits from Parent (which inherits Global 'General')
    let child_cats = db.list_effective_categories(p2).unwrap();
    let names: Vec<_> = child_cats.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"General"));
    assert!(names.contains(&"Backend"));
    assert!(names.contains(&"UI"));
}

#[test]
fn test_project_crud_and_validation() {
    let db = Db::open_in_memory().unwrap();

    // Empty project name fails
    assert!(db.create_project("  ", 10, 10, "desc", true).is_err());

    // Create project successfully
    let pid = db
        .create_project("Project Alpha", 10, 10, "First project", true)
        .unwrap();
    assert!(pid > 0);

    // Duplicate name fails
    assert!(db
        .create_project("Project Alpha", 10, 10, "Duplicate", true)
        .is_err());

    // Get by ID
    let proj = db.get_project_by_id(pid).unwrap().unwrap();
    assert_eq!(proj.id, pid);
    assert_eq!(proj.name, "Project Alpha");
    assert_eq!(proj.status, 10);
    assert_eq!(proj.enabled, 1);
    assert_eq!(proj.view_state, 10);
    assert_eq!(proj.description, "First project");
    assert_eq!(proj.inherit_global, 1);

    // Update project
    assert!(db
        .update_project(
            pid,
            "Project Alpha Updated",
            30,
            false,
            50,
            "Updated desc",
            false
        )
        .is_ok());
    let updated = db.get_project_by_id(pid).unwrap().unwrap();
    assert_eq!(updated.name, "Project Alpha Updated");
    assert_eq!(updated.status, 30);
    assert_eq!(updated.enabled, 0);
    assert_eq!(updated.view_state, 50);
    assert_eq!(updated.description, "Updated desc");
    assert_eq!(updated.inherit_global, 0);

    // Update nonexistent project fails
    assert!(db
        .update_project(9999, "Nonexistent", 10, true, 10, "", true)
        .is_err());
    // Update with empty name fails
    assert!(db.update_project(pid, "", 10, true, 10, "", true).is_err());

    // Add a project-specific category and verify delete_project cleans it up
    let p_cat = db.add_category(pid, "Project-Specific Cat", 0).unwrap();
    assert!(db.get_category_by_id(p_cat).unwrap().is_some());

    // Delete project
    assert!(db.delete_project(pid).is_ok());
    assert!(db.get_project_by_id(pid).unwrap().is_none());
    assert!(db.delete_project(pid).is_err());

    // Verify project-specific category was deleted
    assert!(db.get_category_by_id(p_cat).unwrap().is_none());
    // Verify global category General (project_id = 0) is still intact
    let global_cats = db.list_effective_categories(0).unwrap();
    assert!(global_cats.iter().any(|c| c.name == "General"));
}

#[test]
fn test_subproject_hierarchy_multi_level_cycle_prevention() {
    let db = Db::open_in_memory().unwrap();
    let p1 = db.create_project("A", 10, 10, "", true).unwrap();
    let p2 = db.create_project("B", 10, 10, "", true).unwrap();
    let p3 = db.create_project("C", 10, 10, "", true).unwrap();

    // A -> B -> C
    db.add_subproject(p1, p2, true).unwrap();
    db.add_subproject(p2, p3, true).unwrap();

    // Adding C -> A would form A -> B -> C -> A cycle, must fail
    assert!(db.add_subproject(p3, p1, true).is_err());

    // Adding nonexistent project fails
    assert!(db.add_subproject(p1, 9999, true).is_err());
    assert!(db.add_subproject(9999, p1, true).is_err());

    // Remove subproject
    assert!(db.remove_subproject(p2, p3).is_ok());
    // After removing B -> C, C -> A is now allowed!
    assert!(db.add_subproject(p3, p1, true).is_ok());
}

#[test]
fn test_project_user_management_and_overrides() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("SecuredProj", 10, 50, "", true).unwrap();
    let uid = db
        .create_user("charlie", "Charlie", "c@x.com", "pw", 10, true, false)
        .unwrap();
    let user = db.get_user_by_id(uid).unwrap().unwrap();

    // Initially Viewer (10) has no access to private project
    assert_eq!(db.effective_access_level(&user, pid).unwrap(), 0);

    // Assign Developer (55)
    assert!(db.set_project_user(pid, uid, 55).is_ok());
    assert_eq!(db.effective_access_level(&user, pid).unwrap(), 55);

    // Update to Manager (70)
    assert!(db.set_project_user(pid, uid, 70).is_ok());
    assert_eq!(db.effective_access_level(&user, pid).unwrap(), 70);

    // Remove user from project
    assert!(db.remove_project_user(pid, uid).is_ok());
    assert_eq!(db.effective_access_level(&user, pid).unwrap(), 0);

    // Nonexistent project or user fails
    assert!(db.set_project_user(9999, uid, 55).is_err());
    assert!(db.set_project_user(pid, 9999, 55).is_err());
}

#[test]
fn test_effective_access_level_rules() {
    let db = Db::open_in_memory().unwrap();
    let pub_id = db.create_project("Pub", 10, 10, "", true).unwrap();
    let priv_id = db.create_project("Priv", 10, 50, "", true).unwrap();
    let disabled_id = db.create_project("Disabled", 10, 10, "", true).unwrap();
    db.update_project(disabled_id, "Disabled", 10, false, 10, "", true)
        .unwrap();

    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let dev_id = db
        .create_user("dev", "Dev", "d@x.com", "pw", 55, true, false)
        .unwrap();
    let dev = db.get_user_by_id(dev_id).unwrap().unwrap();
    let mgr_id = db
        .create_user("mgr", "Mgr", "m@x.com", "pw", 70, true, false)
        .unwrap();
    let mgr = db.get_user_by_id(mgr_id).unwrap().unwrap();

    // Rule 1: project_id == 0 returns user.access_level
    assert_eq!(db.effective_access_level(&admin, 0).unwrap(), 90);
    assert_eq!(db.effective_access_level(&dev, 0).unwrap(), 55);
    assert_eq!(db.effective_access_level(&mgr, 0).unwrap(), 70);

    // Rule 2: Administrator always gets 90
    assert_eq!(db.effective_access_level(&admin, pub_id).unwrap(), 90);
    assert_eq!(db.effective_access_level(&admin, priv_id).unwrap(), 90);
    assert_eq!(db.effective_access_level(&admin, disabled_id).unwrap(), 90);

    // Disabled project returns 0 for non-administrator
    assert_eq!(db.effective_access_level(&dev, disabled_id).unwrap(), 0);
    assert_eq!(db.effective_access_level(&mgr, disabled_id).unwrap(), 0);

    // Nonexistent project returns 0 for non-administrator
    assert_eq!(db.effective_access_level(&dev, 99999).unwrap(), 0);

    // auth module wrapper
    assert_eq!(
        crate::auth::effective_access_level(&db, &dev, pub_id).unwrap(),
        55
    );
}

#[test]
fn test_list_visible_projects_and_depths() {
    let db = Db::open_in_memory().unwrap();
    // Create tree:
    // P_Parent (Public)
    //   -> P_Child (Public)
    //        -> P_Grandchild (Public)
    // P_Private (Private)
    //   -> P_PrivateSub (Public)
    let p_parent = db.create_project("Proj Parent", 10, 10, "", true).unwrap();
    let p_child = db.create_project("Proj Child", 10, 10, "", true).unwrap();
    let p_grandchild = db
        .create_project("Proj Grandchild", 10, 10, "", true)
        .unwrap();
    db.add_subproject(p_parent, p_child, true).unwrap();
    db.add_subproject(p_child, p_grandchild, true).unwrap();

    let p_priv = db.create_project("Proj Priv", 10, 50, "", true).unwrap();
    let p_priv_sub = db
        .create_project("Proj Priv Sub", 10, 10, "", true)
        .unwrap();
    db.add_subproject(p_priv, p_priv_sub, true).unwrap();

    let rep_id = db
        .create_user("rep", "Rep", "r@x.com", "pw", 25, true, false)
        .unwrap();
    let rep = db.get_user_by_id(rep_id).unwrap().unwrap();

    let visible = db.list_visible_projects(&rep).unwrap();
    let vis_tuples: Vec<(&str, usize)> =
        visible.iter().map(|(p, d)| (p.name.as_str(), *d)).collect();

    // P_Parent (0) -> P_Child (1) -> P_Grandchild (2)
    // P_Priv is hidden from rep, so P_Priv_Sub becomes root (depth 0)
    assert!(vis_tuples.contains(&("Proj Parent", 0)));
    assert!(vis_tuples.contains(&("Proj Child", 1)));
    assert!(vis_tuples.contains(&("Proj Grandchild", 2)));
    assert!(vis_tuples.contains(&("Proj Priv Sub", 0)));
    assert!(!vis_tuples.iter().any(|(n, _)| *n == "Proj Priv"));

    // Admin sees all, including P_Priv at depth 0 and P_Priv_Sub at depth 1
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let admin_visible = db.list_visible_projects(&admin).unwrap();
    let admin_tuples: Vec<(&str, usize)> = admin_visible
        .iter()
        .map(|(p, d)| (p.name.as_str(), *d))
        .collect();
    assert!(admin_tuples.contains(&("Proj Priv", 0)));
    assert!(admin_tuples.contains(&("Proj Priv Sub", 1)));
}

#[test]
fn test_category_crud_operations() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("CatProj", 10, 10, "", true).unwrap();

    // Empty category name fails
    assert!(db.add_category(pid, "  ", 0).is_err());

    // Add category
    let cid = db.add_category(pid, "Database", 0).unwrap();
    assert!(cid > 0);

    // Duplicate category in same project fails
    assert!(db.add_category(pid, "Database", 0).is_err());

    // Update category
    assert!(db.update_category(cid, "Database Engine", 0).is_ok());
    // Update nonexistent category fails
    assert!(db.update_category(9999, "None", 0).is_err());

    // Delete category
    assert!(db.delete_category(cid).is_ok());
    // Delete nonexistent category fails
    assert!(db.delete_category(cid).is_err());
}

#[test]
fn test_version_crud_and_ordering() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("VerProj", 10, 10, "", true).unwrap();

    // Empty version fails
    assert!(db.add_version(pid, "   ", "desc", true, false).is_err());

    // Add versions
    let v1 = db
        .add_version(pid, "1.0.0", "First release", true, false)
        .unwrap();
    let v2 = db
        .add_version(pid, "2.0.0", "Major update", false, false)
        .unwrap();
    assert!(v1 > 0);
    assert!(v2 > 0);

    // Duplicate version in same project fails
    assert!(db.add_version(pid, "1.0.0", "Dup", true, false).is_err());

    // List versions
    let versions = db.list_versions(pid).unwrap();
    assert_eq!(versions.len(), 2);
    let ver_names: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
    assert!(ver_names.contains(&"1.0.0"));
    assert!(ver_names.contains(&"2.0.0"));

    // Update version
    assert!(db
        .update_version(v2, "2.0.0-rc1", "Release candidate", true, false)
        .is_ok());
    let updated_versions = db.list_versions(pid).unwrap();
    assert!(updated_versions.iter().any(|v| v.version == "2.0.0-rc1"));

    // Update nonexistent fails
    assert!(db.update_version(9999, "none", "", true, false).is_err());

    // Delete version
    assert!(db.delete_version(v1).is_ok());
    assert_eq!(db.list_versions(pid).unwrap().len(), 1);
    // Delete nonexistent fails
    assert!(db.delete_version(v1).is_err());
}

#[test]
fn test_version_date_order_and_backward_compatibility() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("OrderProj", 10, 10, "", true).unwrap();

    // 1. Backward-compatible add_version uses current timestamp (date_order > 0)
    let v1 = db
        .add_version(pid, "1.0.0", "Release 1", true, false)
        .unwrap();
    let ver1 = db.get_version_by_id(v1).unwrap().unwrap();
    assert!(ver1.date_order > 0);

    // 2. add_version_with_order with custom date_order
    let custom_order = 1700000000;
    let v2 = db
        .add_version_with_order(pid, "2.0.0", "Release 2", false, false, Some(custom_order))
        .unwrap();
    let ver2 = db.get_version_by_id(v2).unwrap().unwrap();
    assert_eq!(ver2.date_order, custom_order);

    // 3. Backward-compatible update_version preserves existing date_order
    assert!(db
        .update_version(v2, "2.0.0-rc2", "Updated RC", false, false)
        .is_ok());
    let ver2_after = db.get_version_by_id(v2).unwrap().unwrap();
    assert_eq!(ver2_after.version, "2.0.0-rc2");
    assert_eq!(ver2_after.date_order, custom_order);

    // 4. update_version_with_order updates date_order
    let new_order = 1800000000;
    assert!(db
        .update_version_with_order(v2, "2.0.0-final", "Final", true, false, Some(new_order))
        .is_ok());
    let ver2_final = db.get_version_by_id(v2).unwrap().unwrap();
    assert_eq!(ver2_final.version, "2.0.0-final");
    assert_eq!(ver2_final.date_order, new_order);
    assert_eq!(ver2_final.released, 1);
}

#[test]
fn test_bug_schema_bootstrap_and_crud() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("TestProj", 10, 10, "", true).unwrap();
    let uid = db
        .create_user("alice", "Alice", "a@x.com", "pw", 25, true, false)
        .unwrap();

    let bug_id = db
        .create_bug(
            proj_id,
            uid,
            1,
            "Test summary",
            "Description here",
            "Steps",
            "Additional",
            30,
            50,
            70,
            "1.0",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    assert!(bug_id > 0);

    let bug = db.get_bug(bug_id).unwrap().expect("bug exists");
    assert_eq!(bug.summary, "Test summary");
    assert_eq!(bug.status, 10); // new
    assert_eq!(bug.priority, 30); // normal
    assert_eq!(bug.severity, 50); // minor

    let full = db.get_bug_full(bug_id).unwrap().expect("full bug");
    assert_eq!(full.description, "Description here");
    assert_eq!(full.steps_to_reproduce, "Steps");

    let bugs = db.list_bugs(proj_id, 0, 25).unwrap();
    assert_eq!(bugs.len(), 1);

    // Create private bug
    let priv_bug_id = db
        .create_bug(
            proj_id,
            uid,
            1,
            "Secret bug",
            "Private",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();
    assert!(priv_bug_id > 0);
    // list_bugs returns raw unfiltered bugs (total 2)
    assert_eq!(db.list_bugs(proj_id, 0, 25).unwrap().len(), 2);

    // Update bug fields & verify history only records changed fields
    let changed = db
        .update_bug_fields(
            bug_id,
            uid,
            BugUpdate {
                summary: Some("Updated summary".to_string()),
                priority: Some(30), // unchanged!
                severity: Some(60), // changed 50 -> 60
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(changed.len(), 2);
    assert!(changed.iter().any(|h| h.field_name == "summary"
        && h.old_value == "Test summary"
        && h.new_value == "Updated summary"));
    assert!(changed
        .iter()
        .any(|h| h.field_name == "severity" && h.old_value == "50" && h.new_value == "60"));

    db.delete_bug(bug_id).unwrap();
    assert!(db.get_bug(bug_id).unwrap().is_none());
}

#[test]
fn test_bug_status_workflow_and_history() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let uid = db
        .create_user("dev", "Dev", "d@x.com", "pw", 55, true, false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id, uid, 1, "Summary", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // new -> resolved (with note)
    db.change_bug_status(bug_id, uid, 80, 20, Some("Fixed it"))
        .unwrap();
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 80);
    assert_eq!(bug.resolution, 20);

    let history = db.list_bug_history(bug_id).unwrap();
    let status_change = history.iter().find(|h| h.field_name == "status").unwrap();
    assert_eq!(status_change.old_value, "10");
    assert_eq!(status_change.new_value, "80");

    let notes = db.list_bugnotes(bug_id, 55).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].note, "Fixed it");

    // Private note filtering
    db.add_bugnote(bug_id, uid, "Private internal note", 50)
        .unwrap();
    assert_eq!(db.list_bugnotes(bug_id, 25).unwrap().len(), 1);
    assert_eq!(db.list_bugnotes(bug_id, 55).unwrap().len(), 2);

    // closed -> cannot go to resolved directly
    db.change_bug_status(bug_id, uid, 90, 20, None).unwrap();
    assert!(db.change_bug_status(bug_id, uid, 80, 20, None).is_err());
    // closed -> feedback (20) is allowed
    assert!(db.change_bug_status(bug_id, uid, 20, 30, None).is_ok());
}

#[test]
fn test_bug_relationships_and_duplicate_reverse() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let uid = db
        .create_user("dev", "Dev", "d@x.com", "pw", 55, true, false)
        .unwrap();
    let b1 = db
        .create_bug(
            proj_id, uid, 1, "Bug 1", "", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();
    let b2 = db
        .create_bug(
            proj_id, uid, 1, "Bug 2", "", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // add duplicate_of relationship -> reverse must be auto-created
    db.add_bug_relationship(b2, b1, 1).unwrap(); // b2 is duplicate_of b1
    let rels = db.list_bug_relationships(b1).unwrap();
    assert!(rels
        .iter()
        .any(|r| r.relationship_type == 2 && r.destination_bug_id == b2)); // b1 has_duplicate b2
    assert_eq!(db.get_bug(b2).unwrap().unwrap().duplicate_id, b1);

    // Deleting one side deletes both sides
    let rel_on_b1 = rels[0].id;
    db.delete_bug_relationship(rel_on_b1).unwrap();
    assert!(db.list_bug_relationships(b1).unwrap().is_empty());
    assert!(db.list_bug_relationships(b2).unwrap().is_empty());
    assert_eq!(db.get_bug(b2).unwrap().unwrap().duplicate_id, 0);

    // add related_to -> no reverse auto-created
    db.add_bug_relationship(b1, b2, 0).unwrap();
    let rels2 = db.list_bug_relationships(b2).unwrap();
    assert!(!rels2
        .iter()
        .any(|r| r.source_bug_id == b2 && r.relationship_type == 0));
}

#[test]
fn test_bug_file_attachment_inline_and_cascade() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("P", 10, 10, "", true).unwrap();
    let uid = db
        .create_user("rep", "Rep", "r@x.com", "pw", 25, true, false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id, uid, 1, "Bug", "", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    let content = b"hello file".to_vec();
    let fid = db
        .add_bug_file(bug_id, uid, "test.txt", "text/plain", content.clone(), "")
        .unwrap();
    assert!(fid > 0);

    let files = db.list_bug_files(bug_id).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename, "test.txt");
    assert_eq!(files[0].filesize, 10);

    let (_, fetched) = db.get_bug_file(fid, "").unwrap().unwrap();
    assert_eq!(fetched, content);

    db.delete_bug_file(fid, "").unwrap();
    assert!(db.list_bug_files(bug_id).unwrap().is_empty());

    // Cascade: deleting the bug removes all attachments
    let fid2 = db
        .add_bug_file(bug_id, uid, "test2.txt", "text/plain", b"data".to_vec(), "")
        .unwrap();
    assert!(fid2 > 0);
    db.delete_bug(bug_id).unwrap();
    assert!(db.get_bug_file(fid2, "").unwrap().is_none());
}

#[test]
fn test_tags_custom_fields_filters_search_and_monitors() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("Proj3", 10, 10, "", true).unwrap();
    let uid = db
        .create_user("dev3", "Dev 3", "d3@x.com", "pw", 55, true, false)
        .unwrap();
    let user = db.get_user_by_id(uid).unwrap().unwrap();

    let b1 = db
        .create_bug(
            proj_id,
            uid,
            1,
            "Memory leak in parser",
            "Heap grows unbounded",
            "",
            "",
            40,
            60,
            10,
            "1.0",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let b2 = db
        .create_bug(
            proj_id,
            uid,
            1,
            "UI button alignment",
            "CSS flex issue",
            "",
            "",
            20,
            20,
            10,
            "1.0",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // 1. Tags
    let tag_id = db
        .create_tag(uid, "memory", "Memory related issues")
        .unwrap();
    db.attach_tag_to_bug(b1, tag_id, uid).unwrap();
    let b1_tags = db.list_bug_tags(b1).unwrap();
    assert_eq!(b1_tags.len(), 1);
    assert_eq!(b1_tags[0].tag_name, "memory");

    // 2. Custom Fields
    let cf_id = db
        .create_custom_field(
            "Environment URL",
            0,
            "",
            "https://staging.local",
            "",
            3,
            100,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    db.link_custom_field_to_project(cf_id, proj_id, 1).unwrap();

    // Default value returned before explicit set
    let cf_vals = db.get_custom_field_values_for_bug(b1).unwrap();
    assert_eq!(cf_vals.len(), 1);
    assert_eq!(cf_vals[0].value, "https://staging.local");

    // Min length validation
    assert!(db.set_custom_field_value(cf_id, b1, uid, "ab").is_err());
    db.set_custom_field_value(cf_id, b1, uid, "https://prod.example.com")
        .unwrap();
    let cf_vals2 = db.get_custom_field_values_for_bug(b1).unwrap();
    assert_eq!(cf_vals2[0].value, "https://prod.example.com");

    // History recorded for custom field change
    let hist = db.list_bug_history(b1).unwrap();
    assert!(hist
        .iter()
        .any(|h| h.field_name == "Environment URL" && h.new_value == "https://prod.example.com"));

    // 3. Monitors
    db.add_bug_monitor(b2, uid).unwrap();
    assert_eq!(db.list_bug_monitor_users(b2).unwrap().len(), 1);

    // 4. Full-text Search & Filtering
    let by_kw = db
        .filter_bugs(
            &BugFilter {
                project_id: Some(proj_id),
                search: Some("unbounded".to_string()),
                ..Default::default()
            },
            &user,
        )
        .unwrap();
    assert_eq!(by_kw.len(), 1);
    assert_eq!(by_kw[0].id, b1);

    let by_tag = db
        .filter_bugs(
            &BugFilter {
                project_id: Some(proj_id),
                tag_id: Some(tag_id),
                ..Default::default()
            },
            &user,
        )
        .unwrap();
    assert_eq!(by_tag.len(), 1);
    assert_eq!(by_tag[0].id, b1);

    let by_monitor = db
        .filter_bugs(
            &BugFilter {
                project_id: Some(proj_id),
                monitored_by: Some(uid),
                ..Default::default()
            },
            &user,
        )
        .unwrap();
    assert_eq!(by_monitor.len(), 1);
    assert_eq!(by_monitor[0].id, b2);

    // 5. Saved Filters
    let filter_id = db
        .save_filter(
            uid,
            proj_id,
            true,
            "High Priority Memory",
            &BugFilter {
                project_id: Some(proj_id),
                tag_id: Some(tag_id),
                ..Default::default()
            },
        )
        .unwrap();
    let visible = db.list_visible_filters(uid, proj_id).unwrap();
    assert_eq!(visible.len(), 1);
    let (sf, bf) = db.get_filter(filter_id).unwrap().unwrap();
    assert_eq!(sf.name, "High Priority Memory");
    assert_eq!(bf.tag_id, Some(tag_id));
}

#[test]
fn test_email_queue_time_tracking_and_news() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let dev_id = db
        .create_user("dev1", "Dev One", "dev1@example.com", "pw", 55, true, false)
        .unwrap();
    let watcher_id = db
        .create_user(
            "watcher",
            "Watcher",
            "watcher@example.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    let proj_id = db.create_project("Core", 10, 10, "desc", true).unwrap();

    // 1. Creating bug queues email to reporter (admin: root@localhost)
    let bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Null pointer in parser",
            "Crash on empty token",
            "",
            "",
            30,
            50,
            10,
            "1.0",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(emails.len(), 1);
    assert_eq!(emails[0].email, "root@localhost");
    assert!(emails[0].subject.contains("Null pointer in parser"));

    // Flush queue
    assert_eq!(db.flush_email_queue().unwrap(), 1);
    assert!(db.list_queued_emails().unwrap().is_empty());

    // 2. Add monitor + assign to dev1 -> queues emails to reporter, handler, and monitor
    db.add_bug_monitor(bug_id, watcher_id).unwrap();
    db.assign_bug(bug_id, admin.id, dev_id).unwrap();
    let emails2 = db.list_queued_emails().unwrap();
    let addrs: Vec<&str> = emails2.iter().map(|e| e.email.as_str()).collect();
    assert!(addrs.contains(&"root@localhost"));
    assert!(addrs.contains(&"dev1@example.com"));
    assert!(addrs.contains(&"watcher@example.com"));

    // 3. Time tracking via bugnote
    db.add_bugnote_with_time(bug_id, dev_id, "Investigated and fixed", 10, 95)
        .unwrap();
    let entries = db.list_time_tracking(proj_id).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].minutes, 95);
    assert_eq!(entries[0].username, "dev1");

    // History recorded for time_tracking
    let hist = db.list_bug_history(bug_id).unwrap();
    assert!(hist
        .iter()
        .any(|h| h.field_name == "time_tracking" && h.new_value == "95"));

    // 4. Project News & visibility
    let n_pub = db
        .create_news(
            proj_id,
            admin.id,
            "v1.0 Released",
            "Release notes here",
            true,
            10,
        )
        .unwrap();
    let _n_priv = db
        .create_news(
            proj_id,
            admin.id,
            "Internal Security Audit",
            "Secret body",
            false,
            50,
        )
        .unwrap();

    // Viewer (10) only sees public news
    let viewer_news = db.list_news(proj_id, 10).unwrap();
    assert_eq!(viewer_news.len(), 1);
    assert_eq!(viewer_news[0].id, n_pub);

    // Developer (55) sees both public and private news
    let dev_news = db.list_news(proj_id, 55).unwrap();
    assert_eq!(dev_news.len(), 2);

    // Test get_news and update_news
    let item = db.get_news(n_pub).unwrap().unwrap();
    assert_eq!(item.headline, "v1.0 Released");
    assert_eq!(item.body, "Release notes here");
    assert_eq!(item.announcement, 1);
    assert_eq!(item.view_state, 10);

    let old_modified = item.last_modified;
    assert!(db
        .update_news(n_pub, "v1.0.1 Released", "Updated patch notes", 10, 0)
        .is_ok());
    let updated = db.get_news(n_pub).unwrap().unwrap();
    assert_eq!(updated.headline, "v1.0.1 Released");
    assert_eq!(updated.body, "Updated patch notes");
    assert_eq!(updated.announcement, 0);
    assert!(updated.last_modified >= old_modified);

    // Empty headline or body fails
    assert!(db.update_news(n_pub, "   ", "body", 10, 0).is_err());
    assert!(db.update_news(n_pub, "head", "   ", 10, 0).is_err());
    assert!(db.update_news(99999, "head", "body", 10, 0).is_err());
    assert!(db.get_news(99999).unwrap().is_none());
}

#[test]
fn test_api_tokens_and_plugin_system() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();

    // 1. Create and authenticate API token
    let (tok_id, raw_tok) = db.create_api_token(admin.id, "CI Pipeline").unwrap();
    assert_eq!(raw_tok.len(), 64);

    let tokens = db.list_api_tokens(admin.id).unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].name, "CI Pipeline");
    assert_eq!(tokens[0].date_used, 0);
    // Raw token is never stored in DB
    assert_ne!(tokens[0].hash, raw_tok);

    let authed = db.authenticate_api_token(&raw_tok).unwrap().unwrap();
    assert_eq!(authed.id, admin.id);
    assert!(db.list_api_tokens(admin.id).unwrap()[0].date_used > 0);

    // Revoke token -> subsequent auth fails
    assert!(db.revoke_api_token(tok_id, admin.id).unwrap());
    assert!(db.authenticate_api_token(&raw_tok).unwrap().is_none());

    // 2. Plugin system & protected plugin guard
    let plugins = db.list_plugins().unwrap();
    assert_eq!(plugins.len(), 2);
    assert!(db.is_plugin_enabled("MantisCoreFormatting"));
    assert!(db.is_plugin_enabled("EventAuditLog"));

    // Protected plugin cannot be disabled
    assert!(db
        .set_plugin_enabled("MantisCoreFormatting", false)
        .is_err());

    // EventAuditLog increments plugin_event_audit_count when enabled
    let proj_id = db.create_project("PluginProj", 10, 10, "", true).unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Test plugin event",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    assert_eq!(
        db.get_config("plugin_event_audit_count", 0, 0)
            .unwrap()
            .as_deref(),
        Some("1")
    );

    // Disable EventAuditLog -> subsequent events do not increment counter
    db.set_plugin_enabled("EventAuditLog", false).unwrap();
    assert!(!db.is_plugin_enabled("EventAuditLog"));
    db.add_bugnote(bug_id, admin.id, "Note while plugin disabled", 10)
        .unwrap();
    assert_eq!(
        db.get_config("plugin_event_audit_count", 0, 0)
            .unwrap()
            .as_deref(),
        Some("1")
    );
}

#[test]
fn test_jira_links_crud() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let project_id = db
        .create_project("Jira Project", 10, 10, "Desc", false)
        .unwrap();
    let bug_id = db
        .create_bug(
            project_id, admin.id, 1, "Summary", "Desc", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Check initially none
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());

    // Create link
    let link_id = db
        .create_jira_link(
            bug_id,
            "PROJ-101",
            "https://jira.example.com/browse/PROJ-101",
            "Bug",
            "In Progress",
            admin.id,
        )
        .unwrap();
    assert!(link_id > 0);

    // Fetch link
    let link = db
        .get_jira_link_by_bug_id(bug_id)
        .unwrap()
        .expect("link should exist");
    assert_eq!(link.bug_id, bug_id);
    assert_eq!(link.jira_key, "PROJ-101");
    assert_eq!(link.jira_url, "https://jira.example.com/browse/PROJ-101");
    assert_eq!(link.issue_type, "Bug");
    assert_eq!(link.jira_status, "In Progress");
    assert_eq!(link.created_by, admin.id);

    // Duplicate create fails due to UNIQUE constraint on bug_id
    assert!(db
        .create_jira_link(
            bug_id,
            "PROJ-102",
            "https://jira.example.com/browse/PROJ-102",
            "Task",
            "To Do",
            admin.id,
        )
        .is_err());

    // Delete link
    let deleted = db.delete_jira_link_by_bug_id(bug_id).unwrap();
    assert!(deleted);
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());
}

#[test]
fn test_jira_sync_success_updates_status_and_adds_history() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let project_id = db
        .create_project("Jira Project", 10, 10, "Desc", false)
        .unwrap();
    let bug_id = db
        .create_bug(
            project_id, admin.id, 1, "Summary", "Desc", "", "", 10, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // 1. Initial link with "In Progress" status
    db.record_jira_sync_success(
        bug_id,
        "JBC-10",
        "https://jira.example.com/browse/JBC-10",
        "Story",
        "In Progress",
        admin.id,
        "administrator",
        true,
    )
    .unwrap();

    let link = db.get_jira_link_by_bug_id(bug_id).unwrap().unwrap();
    assert_eq!(link.jira_key, "JBC-10");
    assert_eq!(link.jira_status, "In Progress");
    assert_eq!(link.issue_type, "Story");

    // Bug status should have transitioned to 50 (assigned)
    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.status, 50);

    // 2. Refresh sync with "Done" status
    db.record_jira_sync_success(
        bug_id,
        "JBC-10",
        "https://jira.example.com/browse/JBC-10",
        "Story",
        "Done",
        admin.id,
        "administrator",
        false,
    )
    .unwrap();

    let link2 = db.get_jira_link_by_bug_id(bug_id).unwrap().unwrap();
    assert_eq!(link2.jira_status, "Done");

    let bug2 = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug2.status, 80); // resolved

    // 3. Unlink
    let unlinked = db
        .delete_jira_link_with_note(bug_id, admin.id, "administrator")
        .unwrap();
    assert!(unlinked);
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());

    // Check bugnotes
    let notes = db.list_bugnotes(bug_id, 90).unwrap();
    assert!(notes
        .iter()
        .any(|n| n.note.contains("Unlinked Jira issue JBC-10")));
}

#[test]
fn test_jira_config_resolution() {
    let db = Db::open_in_memory().unwrap();
    let project_id = db
        .create_project("Test Project", 10, 10, "Desc", false)
        .unwrap();

    // Initially disabled with defaults
    let conf = db.get_jira_config(project_id).unwrap();
    assert!(!conf.enabled);
    assert_eq!(conf.default_issue_type, "Bug");

    // Save global config
    db.save_jira_global_config(
        true,
        "https://test.atlassian.net",
        "bot@test.com",
        Some("secret-token"),
        "Task",
        "DEFAULT_PROJ",
    )
    .unwrap();

    let conf2 = db.get_jira_config(project_id).unwrap();
    assert!(conf2.enabled);
    assert_eq!(conf2.base_url, "https://test.atlassian.net");
    assert_eq!(conf2.email, "bot@test.com");
    assert_eq!(conf2.api_token, "secret-token");
    assert_eq!(conf2.default_issue_type, "Task");
    assert_eq!(conf2.project_key, "DEFAULT_PROJ");

    // Override project key for specific project
    db.set_jira_project_mapping(project_id, "SPECIFIC_PROJ")
        .unwrap();
    let conf3 = db.get_jira_config(project_id).unwrap();
    assert_eq!(conf3.project_key, "SPECIFIC_PROJ");

    // Another project falls back to default
    let conf_other = db.get_jira_config(9999).unwrap();
    assert_eq!(conf_other.project_key, "DEFAULT_PROJ");
}

#[test]
fn test_resolve_jira_api_token_env_priority() {
    assert_eq!(
        Db::resolve_jira_api_token(Some("  env_tok_123  "), Some("db_tok_456".to_string())),
        "env_tok_123"
    );
    assert_eq!(
        Db::resolve_jira_api_token(Some("   "), Some("db_tok_456".to_string())),
        "db_tok_456"
    );
    assert_eq!(
        Db::resolve_jira_api_token(None, Some("db_tok_456".to_string())),
        "db_tok_456"
    );
    assert_eq!(Db::resolve_jira_api_token(None, None), "");
}

#[test]
fn test_signup_user_success_and_validations() {
    let db = Db::open_in_memory().unwrap();

    // 1. Successful signup
    let new_user = db
        .signup_user("johndoe", "John Doe", "john@example.com", "secret123")
        .expect("Signup should succeed");
    assert_eq!(new_user.username, "johndoe");
    assert_eq!(new_user.realname, "John Doe");
    assert_eq!(new_user.email, "john@example.com");
    assert_eq!(new_user.access_level, 25); // Reporter
    assert_eq!(new_user.enabled, 1);
    assert_eq!(new_user.protected, 0);

    // Verification via get_user_by_email
    let by_email = db.get_user_by_email("john@example.com").unwrap().unwrap();
    assert_eq!(by_email.id, new_user.id);
    assert_eq!(by_email.username, "johndoe");

    // 2. Duplicate username rejection
    let err_dup_user = db.signup_user("johndoe", "Another John", "john2@example.com", "secret123");
    assert!(err_dup_user.is_err());
    assert!(err_dup_user.unwrap_err().contains("already taken"));

    // Case-insensitive duplicate username
    let err_dup_user_case =
        db.signup_user("JohnDoe", "Another John", "john3@example.com", "secret123");
    assert!(err_dup_user_case.is_err());

    // 3. Duplicate email rejection
    let err_dup_email = db.signup_user("johndoe2", "John 2", "john@example.com", "secret123");
    assert!(err_dup_email.is_err());
    assert!(err_dup_email.unwrap_err().contains("already in use"));

    // 4. Invalid username format or empty
    let err_empty_user = db.signup_user("   ", "Name", "empty@example.com", "secret123");
    assert!(err_empty_user.is_err());

    // 5. Invalid email format
    let err_invalid_email = db.signup_user("validuser", "Name", "not-an-email", "secret123");
    assert!(err_invalid_email.is_err());

    // 6. Password too short (< 6 chars)
    let err_short_pass = db.signup_user("shortpassuser", "Name", "short@example.com", "12345");
    assert!(err_short_pass.is_err());
}

#[test]
fn test_is_signup_allowed_default_and_override() {
    let db = Db::open_in_memory().unwrap();
    // Default should be allowed
    assert!(db.is_signup_allowed());

    // Explicitly disable signup
    db.set_config("allow_signup", 0, 0, "0").unwrap();
    assert!(!db.is_signup_allowed());

    // Re-enable
    db.set_config("allow_signup", 0, 0, "1").unwrap();
    assert!(db.is_signup_allowed());
}

#[test]
fn test_jira_epics_cache() {
    let db = Db::open_in_memory().unwrap();
    let epics = vec![
        crate::models::JiraEpic {
            key: "JBC-1".to_string(),
            summary: "Epic One".to_string(),
        },
        crate::models::JiraEpic {
            key: "JBC-2".to_string(),
            summary: "Epic Two".to_string(),
        },
    ];

    // Initial cache should be None
    assert!(db.get_cached_jira_epics("JBC", 86400).unwrap().is_none());

    // Save cache
    db.save_jira_epics_cache("JBC", &epics).unwrap();

    // Cache hit within 86400s
    let cached = db.get_cached_jira_epics("JBC", 86400).unwrap().unwrap();
    assert_eq!(cached.len(), 2);
    assert_eq!(cached[0].key, "JBC-1");
    assert_eq!(cached[0].summary, "Epic One");
    assert_eq!(cached[1].key, "JBC-2");

    // Cache miss with max_age_secs = 0 (expired)
    // Wait: saturating_sub(updated_at) might be 0 if same second, so test with negative or check summary
    let summary = db.list_cached_jira_epics_summary().unwrap();
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].0, "JBC");
    assert_eq!(summary[0].1, 2);
    assert!(summary[0].2 > 0);

    // Update with empty list
    db.save_jira_epics_cache("JBC", &[]).unwrap();
    let updated = db.get_cached_jira_epics("JBC", 86400).unwrap().unwrap();
    assert_eq!(updated.len(), 0);

    // Test TTL expiry
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    db.save_jira_epics_cache_with_ttl("JBC", &epics, now, now - 10)
        .unwrap();
    // Normal query should consider it expired
    assert!(db.get_cached_jira_epics("JBC", 86400).unwrap().is_none());
    // Fallback query (i64::MAX) should still retrieve it
    assert_eq!(
        db.get_cached_jira_epics("JBC", i64::MAX)
            .unwrap()
            .unwrap()
            .len(),
        2
    );
    // Summary still shows updated_at as now, not 24h ago
    let summary2 = db.list_cached_jira_epics_summary().unwrap();
    assert_eq!(summary2[0].2, now);
}

#[test]
fn test_jira_status_mappings() {
    let db = Db::open_in_memory().unwrap();

    // Default seeded mappings
    let initial = db.list_jira_status_mappings().unwrap();
    assert!(!initial.is_empty());
    assert_eq!(db.resolve_jira_status_to_mantis("To Do"), 30);
    assert_eq!(db.resolve_jira_status_to_mantis("In Progress"), 50);
    assert_eq!(db.resolve_jira_status_to_mantis("In Review"), 20);
    assert_eq!(db.resolve_jira_status_to_mantis("Done"), 80);

    // Save custom mappings with case-insensitive duplicates
    let custom = vec![
        ("Backlog".to_string(), 10),
        ("backlog".to_string(), 20), // Duplicate case-insensitive -> ignored
        ("In Dev".to_string(), 50),
        ("QA Passed".to_string(), 80),
        ("Live Released".to_string(), 90),
    ];
    db.save_jira_status_mappings(&custom).unwrap();

    let saved = db.list_jira_status_mappings().unwrap();
    assert_eq!(saved.len(), 4);

    assert_eq!(db.resolve_jira_status_to_mantis("Backlog"), 10);
    assert_eq!(db.resolve_jira_status_to_mantis("in dev"), 50); // case insensitive
    assert_eq!(db.resolve_jira_status_to_mantis("QA Passed"), 80);
    assert_eq!(db.resolve_jira_status_to_mantis("LIVE RELEASED"), 90);

    // Unmapped fallback heuristic
    assert_eq!(db.resolve_jira_status_to_mantis("Resolved"), 80);
}

#[test]
fn test_jira_sync_preserves_resolved_bugs_and_avoids_duplicate_notes() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_id(1).unwrap().unwrap();
    let proj_id = db
        .create_project("Sync Test", 10, 10, "desc", false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Resolved Bug",
            "Desc",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Mark bug as resolved (80)
    db.change_bug_status(bug_id, admin.id, 80, 20, None)
        .unwrap();
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().status, 80);

    // Initial sync link with Jira ticket having status "To Do" (which maps to 30)
    db.record_jira_sync_success(
        bug_id,
        "PROJ-100",
        "https://jira.example.com/browse/PROJ-100",
        "Bug",
        "To Do",
        admin.id,
        "administrator",
        true,
    )
    .unwrap();

    // Verify bug status was NOT downgraded from 80 back to 30
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().status, 80);

    // Verify exactly 1 note was added for initial link
    let notes = db.list_bugnotes(bug_id, 90).unwrap();
    assert_eq!(notes.len(), 1);

    // Re-sync with exact same status & key
    db.record_jira_sync_success(
        bug_id,
        "PROJ-100",
        "https://jira.example.com/browse/PROJ-100",
        "Bug",
        "To Do",
        admin.id,
        "administrator",
        false,
    )
    .unwrap();

    // Verify no redundant note was added
    let notes_after = db.list_bugnotes(bug_id, 90).unwrap();
    assert_eq!(notes_after.len(), 1);

    // Empty Jira status should NOT alter Mantis bug status
    db.record_jira_sync_success(
        bug_id,
        "PROJ-100",
        "https://jira.example.com/browse/PROJ-100",
        "Bug",
        "",
        admin.id,
        "administrator",
        false,
    )
    .unwrap();
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().status, 80);

    // Closed bug (90) should NOT move backwards to resolved (80) when syncing with "Done"
    db.change_bug_status(bug_id, admin.id, 90, 20, None)
        .unwrap();
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().status, 90);

    db.record_jira_sync_success(
        bug_id,
        "PROJ-100",
        "https://jira.example.com/browse/PROJ-100",
        "Bug",
        "Done",
        admin.id,
        "administrator",
        false,
    )
    .unwrap();
    assert_eq!(db.get_bug(bug_id).unwrap().unwrap().status, 90);
}

#[test]
fn test_jira_claim_pending_concurrency() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_id(1).unwrap().unwrap();
    let proj_id = db
        .create_project("Claim Project", 10, 10, "desc", false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Claim Bug",
            "Desc",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // First claim succeeds
    assert!(db.claim_jira_link_pending(bug_id, admin.id).unwrap());

    // Concurrent second claim for same bug fails immediately
    assert!(!db.claim_jira_link_pending(bug_id, admin.id).unwrap());

    // Pending link is hidden from standard get_jira_link_by_bug_id
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());

    // Release pending link allows new claim
    db.release_jira_link_pending(bug_id).unwrap();
    assert!(db.claim_jira_link_pending(bug_id, admin.id).unwrap());

    // Success sync overrides pending link
    db.record_jira_sync_success(
        bug_id,
        "PROJ-42",
        "https://jira/PROJ-42",
        "Bug",
        "Open",
        admin.id,
        "administrator",
        true,
    )
    .unwrap();

    let link = db.get_jira_link_by_bug_id(bug_id).unwrap().unwrap();
    assert_eq!(link.jira_key, "PROJ-42");
    assert_eq!(link.created_by, admin.id);
    let orig_created_at = link.created_at;

    // Resyncing by user 2 should update status and ticket info, but preserve created_by and created_at
    let user2_id = db
        .create_user(
            "user2",
            "User Two",
            "u2@example.com",
            "pass2",
            70,
            false,
            false,
        )
        .unwrap();
    db.record_jira_sync_success(
        bug_id,
        "PROJ-42",
        "https://jira/PROJ-42",
        "Bug",
        "In Progress",
        user2_id,
        "user2",
        false,
    )
    .unwrap();
    let re_synced_link = db.get_jira_link_by_bug_id(bug_id).unwrap().unwrap();
    assert_eq!(re_synced_link.created_by, admin.id);
    assert_eq!(re_synced_link.created_at, orig_created_at);

    // Once fully linked, claiming pending returns false
    assert!(!db.claim_jira_link_pending(bug_id, admin.id).unwrap());
}

#[test]
fn test_jira_db_schema_migration_from_old_schema() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    // Simulate old schema on develop before migration:
    // 1. mantis_jira_links_table without UNIQUE on bug_id, and with non-unique idx_jira_links_bug_id
    // 2. mantis_jira_status_mapping_table without COLLATE NOCASE
    conn.execute_batch(
        "CREATE TABLE mantis_user_table (id INTEGER PRIMARY KEY, username TEXT, access_level INTEGER);
         INSERT INTO mantis_user_table VALUES (1, 'administrator', 90);
         CREATE TABLE mantis_project_table (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE mantis_bug_table (id INTEGER PRIMARY KEY, project_id INT, reporter_id INT, status INT, last_updated INT);
         INSERT INTO mantis_bug_table VALUES (1, 1, 1, 50, 0);
         CREATE TABLE mantis_bugnote_table (id INTEGER PRIMARY KEY, bug_id INT, reporter_id INT, bugnote_text_id INT, view_state INT, date_submitted INT, last_modified INT, note_type INT, note_attr TEXT, time_tracking INT);
         CREATE TABLE mantis_bugnote_text_table (id INTEGER PRIMARY KEY, note TEXT);
         CREATE TABLE mantis_bug_history_table (id INTEGER PRIMARY KEY, user_id INT, bug_id INT, date_modified INT, field_name TEXT, old_value TEXT, new_value TEXT, type INT);
         CREATE TABLE mantis_jira_links_table (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             bug_id INTEGER NOT NULL,
             jira_key TEXT NOT NULL,
             jira_url TEXT NOT NULL,
             issue_type TEXT NOT NULL DEFAULT 'Bug',
             created_by INTEGER NOT NULL,
             created_at INTEGER NOT NULL
         );
         CREATE INDEX idx_jira_links_bug_id ON mantis_jira_links_table(bug_id);
         INSERT INTO mantis_jira_links_table (bug_id, jira_key, jira_url, issue_type, created_by, created_at)
         VALUES (1, 'OLD-1', 'http://jira/OLD-1', 'Bug', 1, 100);
         INSERT INTO mantis_jira_links_table (bug_id, jira_key, jira_url, issue_type, created_by, created_at)
         VALUES (1, 'OLD-2', 'http://jira/OLD-2', 'Bug', 1, 200);

         CREATE TABLE mantis_jira_status_mapping_table (
             jira_status TEXT PRIMARY KEY,
             mantis_status INTEGER NOT NULL
         );
         INSERT INTO mantis_jira_status_mapping_table VALUES ('Done', 80);"
    ).unwrap();

    let db = Db::from_connection(conn);
    db.migrate().unwrap();

    // Verify duplicates were purged and newest link remains
    let link = db.get_jira_link_by_bug_id(1).unwrap().unwrap();
    assert_eq!(link.jira_key, "OLD-2");

    // Verify record_jira_sync_success UPSERT works without ON CONFLICT error
    db.record_jira_sync_success(
        1,
        "NEW-1",
        "http://jira/NEW-1",
        "Bug",
        "Done",
        1,
        "administrator",
        false,
    )
    .unwrap();

    let updated_link = db.get_jira_link_by_bug_id(1).unwrap().unwrap();
    assert_eq!(updated_link.jira_key, "NEW-1");
    assert_eq!(updated_link.jira_status, "Done");

    // Verify COLLATE NOCASE works on migrated table
    assert_eq!(db.resolve_jira_status_to_mantis("done"), 80);
}

#[test]
fn test_email_templates_detailed() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let dev_id = db
        .create_user("dev1", "Dev One", "dev1@example.com", "pw", 55, true, false)
        .unwrap();
    let proj_id = db
        .create_project("Core System", 10, 10, "desc", true)
        .unwrap();

    // 1. Create bug -> New Issue email
    let bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Null pointer in parser",
            "Crash on empty token",
            "1. Open page\n2. Click filter",
            "",
            30,
            50,
            10,
            "1.0",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(emails.len(), 1);
    assert_eq!(
        emails[0].subject,
        format!("[Core System {:07}] New: Null pointer in parser", bug_id)
    );
    assert!(emails[0]
        .body
        .contains("The following issue has been SUBMITTED."));
    assert!(emails[0]
        .body
        .contains(&format!("/view_bug_page/{}", bug_id)));
    assert!(emails[0]
        .body
        .contains("Reported By:                administrator"));
    assert!(emails[0]
        .body
        .contains("Project:                    Core System"));
    assert!(emails[0]
        .body
        .contains(&format!("Issue ID:                   {}", bug_id)));
    assert!(emails[0]
        .body
        .contains("Category:                   General"));
    assert!(emails[0]
        .body
        .contains("Reproducibility:            always"));
    assert!(emails[0].body.contains("Severity:                   minor"));
    assert!(emails[0]
        .body
        .contains("Priority:                   normal"));
    assert!(emails[0].body.contains("Status:                     new"));
    assert!(emails[0].body.contains("Date Submitted:             "));
    assert!(emails[0].body.contains("Last Modified:              "));
    assert!(emails[0]
        .body
        .contains("Summary:                    Null pointer in parser"));
    assert!(emails[0]
        .body
        .contains("Description:\nCrash on empty token"));
    assert!(emails[0]
        .body
        .contains("Steps to Reproduce:\n1. Open page\n2. Click filter"));
    assert!(emails[0].body.contains("Issue History"));
    assert!(emails[0].body.contains("New Issue"));

    db.flush_email_queue().unwrap();

    // 2. Assign bug -> Assigned email
    db.assign_bug(bug_id, admin.id, dev_id).unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(emails.len(), 2); // reporter & handler
    assert_eq!(
        emails[0].subject,
        format!(
            "[Core System {:07}] Assigned to dev1: Null pointer in parser",
            bug_id
        )
    );
    assert!(emails[0]
        .body
        .contains("The following issue has been ASSIGNED."));
    assert!(emails[0]
        .body
        .contains("Assigned To:                dev1 (Dev One)"));
    assert!(emails[0].body.contains("Assigned To              => dev1"));
    assert!(emails[0]
        .body
        .contains("Status                   new => assigned"));

    db.flush_email_queue().unwrap();

    // 3. Status change -> Status Changed email (only 1 email per recipient, no duplicate note email)
    db.change_bug_status(bug_id, dev_id, 80, 20, Some("Fixed in patch v1.2"))
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(emails.len(), 2); // reporter & handler, neither receives duplicate note email
    assert!(emails
        .iter()
        .all(|e| e.subject.contains("Status changed to resolved")));
    let status_email = &emails[0];
    assert_eq!(
        status_email.subject,
        format!(
            "[Core System {:07}] Status changed to resolved: Null pointer in parser",
            bug_id
        )
    );
    assert!(status_email
        .body
        .contains("The following issue has been RESOLVED."));
    assert!(status_email
        .body
        .contains("Status:                     resolved"));
    assert!(status_email
        .body
        .contains("Resolution:                 fixed"));
    assert!(status_email.body.contains("Fixed in patch v1.2"));
    assert!(status_email
        .body
        .contains("Status                   assigned => resolved"));
    assert!(status_email
        .body
        .contains("Resolution               open => fixed"));

    db.flush_email_queue().unwrap();

    // 4. Note added -> Note added email
    db.add_bugnote(bug_id, admin.id, "Please verify on staging", 10)
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    let note_email = emails
        .iter()
        .find(|e| e.subject.contains("Note added"))
        .expect("Note added email not found");
    assert_eq!(
        note_email.subject,
        format!(
            "[Core System {:07}] Note added by administrator: Null pointer in parser",
            bug_id
        )
    );
    assert!(note_email
        .body
        .contains("A NOTE has been added to this issue."));
    assert!(note_email.body.contains("Please verify on staging"));

    db.flush_email_queue().unwrap();

    // 5. Unassigning issue (handler_id = 0)
    db.assign_bug(bug_id, admin.id, 0).unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(
        emails[0].subject,
        format!(
            "[Core System {:07}] Unassigned: Null pointer in parser",
            bug_id
        )
    );
    assert!(emails[0]
        .body
        .contains("The following issue has been UNASSIGNED."));
    assert!(emails[0].body.contains("Assigned To:                \n"));
    assert!(emails[0].body.contains("dev1 =>"));

    db.flush_email_queue().unwrap();

    // 6. User with empty realname (should format as username, no empty parentheses)
    let plain_user_id = db
        .create_user("plainuser", "", "plain@example.com", "pw", 55, true, false)
        .unwrap();
    db.assign_bug(bug_id, admin.id, plain_user_id).unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(
        emails[0].subject,
        format!(
            "[Core System {:07}] Assigned to plainuser: Null pointer in parser",
            bug_id
        )
    );
    assert!(emails[0]
        .body
        .contains("Assigned To:                plainuser\n"));

    db.flush_email_queue().unwrap();

    // 7. Private note permissions: reporter with access 25 must NOT receive private note email
    let reporter_user_id = db
        .create_user(
            "lowreporter",
            "Low Reporter",
            "low@example.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    let priv_bug_id = db
        .create_bug(
            proj_id,
            reporter_user_id,
            1,
            "Bug with private note",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.assign_bug(priv_bug_id, admin.id, dev_id).unwrap();
    db.flush_email_queue().unwrap();

    // Add private note (view_state = 50)
    db.add_bugnote(priv_bug_id, dev_id, "Secret internal discussion", 50)
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    let recipients: Vec<&str> = emails.iter().map(|e| e.email.as_str()).collect();
    // admin (90) and dev1 (55) can receive it, lowreporter (25) cannot!
    assert!(!recipients.contains(&"low@example.com"));
    assert!(recipients.contains(&"dev1@example.com"));

    db.flush_email_queue().unwrap();

    // 8. Subject header injection prevention: \r and \n are stripped
    {
        let conn = db.conn();
        db.enqueue_email(
            &conn,
            "test@example.com",
            "Header\r\nInjection: Bad",
            "body",
        )
        .unwrap();
    }
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(emails[0].subject, "HeaderInjection: Bad");
}

#[test]
fn test_email_review_comments_improvements() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let dev1_id = db
        .create_user("dev1", "Dev One", "dev1@example.com", "pw", 55, true, false)
        .unwrap();
    let dev2_id = db
        .create_user("dev2", "Dev Two", "dev2@example.com", "pw", 55, true, false)
        .unwrap();
    let reporter_id = db
        .create_user(
            "reporter1",
            "Reporter One",
            "rep1@example.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    let proj_id = db
        .create_project("Project Alpha", 10, 10, "Alpha proj", true)
        .unwrap();

    // 1. Previous handler notification on reassignment
    let bug_id = db
        .create_bug(
            proj_id,
            reporter_id,
            1,
            "Reassignment Bug",
            "Initial desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10, // public bug
        )
        .unwrap();
    // Assign to dev1
    db.assign_bug(bug_id, admin.id, dev1_id).unwrap();
    db.flush_email_queue().unwrap();

    // Now reassign to dev2
    db.assign_bug(bug_id, admin.id, dev2_id).unwrap();
    let emails = db.list_queued_emails().unwrap();
    let recipients: Vec<&str> = emails.iter().map(|e| e.email.as_str()).collect();
    // Previous handler (dev1@example.com), new handler (dev2@example.com), and reporter (rep1@example.com) should receive email!
    assert!(recipients.contains(&"dev1@example.com"));
    assert!(recipients.contains(&"dev2@example.com"));
    assert!(recipients.contains(&"rep1@example.com"));
    db.flush_email_queue().unwrap();

    // 2. Private bug (view_state = 50) check: low access user (access 25) does NOT receive emails unless reporter or handler
    let priv_bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Top Secret Bug",
            "Confidential",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            50, // private bug
        )
        .unwrap();
    // Monitor by reporter1 (access 25)
    db.add_bug_monitor(priv_bug_id, reporter_id).unwrap();
    // Monitor by dev1 (access 55)
    db.add_bug_monitor(priv_bug_id, dev1_id).unwrap();
    db.flush_email_queue().unwrap();

    // Status change on private bug
    db.change_bug_status(priv_bug_id, admin.id, 50, 10, None)
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    let recipients: Vec<&str> = emails.iter().map(|e| e.email.as_str()).collect();
    // dev1 (access 55) and admin receive; reporter1 (access 25) must NOT receive because bug is private!
    assert!(recipients.contains(&"dev1@example.com"));
    assert!(!recipients.contains(&"rep1@example.com"));
    db.flush_email_queue().unwrap();

    // 3. Neutral subject if status unchanged
    // A: Status same (50 -> 50), resolution changed (10 -> 20 fixed)
    db.change_bug_status(bug_id, admin.id, 50, 20, None)
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert!(emails[0]
        .subject
        .contains("Resolution changed to fixed: Reassignment Bug"));
    db.flush_email_queue().unwrap();

    // B: Status same (50 -> 50) and resolution same (20 -> 20) with no note -> returns early without sending email!
    db.change_bug_status(bug_id, admin.id, 50, 20, None)
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert_eq!(emails.len(), 0);

    // C: Status same (50 -> 50) and resolution same (20 -> 20) WITH note -> sends "Updated:" email
    db.change_bug_status(bug_id, admin.id, 50, 20, Some("Note with update"))
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    assert!(emails[0].subject.contains("Updated: Reassignment Bug"));
    db.flush_email_queue().unwrap();

    // 4. Private note exclusion from history in subsequent email
    let note_id = db
        .add_bugnote(bug_id, dev2_id, "Super secret dev note", 50)
        .unwrap(); // view_state = 50
    db.flush_email_queue().unwrap();

    // Trigger another update (e.g. status change)
    db.change_bug_status(bug_id, dev2_id, 80, 20, None).unwrap();
    let emails = db.list_queued_emails().unwrap();
    let email_body = &emails[0].body;
    // Issue history in this email must NOT leak the private note
    let history_note_ref = format!("{:07}", note_id);
    assert!(
        !email_body.contains(&history_note_ref),
        "Email history must not reference private note ID"
    );
    assert!(!email_body.contains("Super secret dev note"));
    db.flush_email_queue().unwrap();

    // 5. Reporter of private bug CAN view and receive email for their own private bug
    let rep_priv_bug = db
        .create_bug(
            proj_id,
            reporter_id, // reporter has access 25
            1,
            "Private Reported Bug",
            "Confidential by reporter",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            50, // private bug
        )
        .unwrap();
    let emails = db.list_queued_emails().unwrap();
    let recipients: Vec<&str> = emails.iter().map(|e| e.email.as_str()).collect();
    assert!(recipients.contains(&"rep1@example.com"));
    let bug_obj = db.get_bug(rep_priv_bug).unwrap().unwrap();
    assert!(Db::can_view_bug(reporter_id, 25, &bug_obj));
    assert!(!Db::can_view_bug(999, 25, &bug_obj)); // another low-access user cannot view it

    // Handler can also view it even if access < 55
    let handler_id = db
        .create_user(
            "priv_handler",
            "Priv Handler",
            "handler@example.com",
            "password",
            25, // Reporter access level
            true,
            false,
        )
        .unwrap();
    db.assign_bug(rep_priv_bug, reporter_id, handler_id)
        .unwrap();
    let bug_obj_handled = db.get_bug(rep_priv_bug).unwrap().unwrap();
    assert!(Db::can_view_bug(handler_id, 25, &bug_obj_handled));

    // Verify filter_bugs visibility
    let handler_user = db.get_user_by_id(handler_id).unwrap().unwrap();
    let rep_user = db.get_user_by_id(reporter_id).unwrap().unwrap();
    let other_user = db
        .get_user_by_id(
            db.create_user("other", "Other", "o@ex.com", "pw", 25, true, false)
                .unwrap(),
        )
        .unwrap()
        .unwrap();

    let filter = crate::models::BugFilter {
        project_id: Some(proj_id),
        ..Default::default()
    };
    let handler_bugs = db.filter_bugs(&filter, &handler_user).unwrap();
    assert!(handler_bugs.iter().any(|b| b.id == rep_priv_bug));

    let rep_bugs = db.filter_bugs(&filter, &rep_user).unwrap();
    assert!(rep_bugs.iter().any(|b| b.id == rep_priv_bug));

    let other_bugs = db.filter_bugs(&filter, &other_user).unwrap();
    assert!(!other_bugs.iter().any(|b| b.id == rep_priv_bug));

    // Test DEFAULT_MANTIS_ADDR constant and base URL fallback
    assert_eq!(crate::DEFAULT_MANTIS_ADDR, "127.0.0.1:3000");
    let fallback_url = db.base_url();
    assert!(fallback_url.contains("127.0.0.1:3000") || fallback_url.starts_with("http"));
    db.flush_email_queue().unwrap();

    // 6. Test history limit keeps the NEWEST rows in email
    for i in 1..=105 {
        let note_text = format!("Sequential note {:03}", i);
        db.change_bug_status(bug_id, admin.id, 50, 20, Some(&note_text))
            .unwrap();
    }
    let emails = db.list_queued_emails().unwrap();
    let latest_email = &emails[0];
    // The email's Issue History table must include the newest changes (e.g. note 105)
    assert!(latest_email.body.contains("Sequential note 105"));
}

#[test]
fn test_email_templates_and_notification_full_coverage() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let proj_id = db
        .create_project("Full Coverage Proj", 10, 10, "desc", true)
        .unwrap();

    // 1. build_mantis_email_body for non-existent bug
    {
        let conn = db.conn();
        let empty_body = db
            .build_mantis_email_body(&conn, 999999, "Header", None)
            .unwrap();
        assert_eq!(empty_body, "");

        // 2. enqueue_email with whitespace/empty email returns Ok(0)
        let res = db.enqueue_email(&conn, "   ", "Subject", "Body").unwrap();
        assert_eq!(res, 0);

        // 3. notify_bug_event_filtered on non-existent bug returns early
        db.notify_bug_event_filtered(&conn, 999999, 0, &[], "Subject", "Body");
    }

    // 4. Create bug with resolution == 0, additional_information, and steps_to_reproduce
    let bug_id = db
        .create_bug(
            proj_id,
            admin.id,
            1,
            "Detailed Bug",
            "Initial description",
            "1. Step A\n2. Step B",
            "Extra system info",
            999, // invalid reproducibility -> fallback
            999, // invalid severity -> fallback
            999, // invalid priority -> fallback
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // 5. Populate history with all remaining field types to cover format_history_field_change
    let body = {
        let conn = db.conn();
        db.record_history(&conn, bug_id, admin.id, "priority", "30", "40", 0)
            .unwrap();
        db.record_history(&conn, bug_id, admin.id, "severity", "50", "60", 0)
            .unwrap();
        db.record_history(&conn, bug_id, admin.id, "reproducibility", "10", "30", 0)
            .unwrap();
        db.record_history(
            &conn,
            bug_id,
            admin.id,
            "file_added",
            "",
            "attached_file.txt",
            0,
        )
        .unwrap();
        db.record_history(
            &conn,
            bug_id,
            admin.id,
            "file_deleted",
            "old_file.txt",
            "",
            0,
        )
        .unwrap();
        db.record_history(&conn, bug_id, admin.id, "category_id", "1", "2", 0)
            .unwrap();
        db.record_history(&conn, bug_id, admin.id, "target_version", "1.0", "1.1", 0)
            .unwrap();
        db.record_history(&conn, bug_id, admin.id, "fixed_in_version", "", "1.0", 0)
            .unwrap();
        db.record_history(&conn, bug_id, admin.id, "version", "1.0", "2.0", 0)
            .unwrap();
        db.record_history(&conn, bug_id, admin.id, "view_state", "10", "50", 0)
            .unwrap();
        db.record_history(
            &conn,
            bug_id,
            admin.id,
            "custom_field_x",
            "OldVal",
            "NewVal",
            0,
        )
        .unwrap();
        // Test handler_id where user is missing in DB (user#9999 fallback)
        db.record_history(&conn, bug_id, admin.id, "handler_id", "9998", "9999", 0)
            .unwrap();
        // Test format_transition with unparseable string values
        db.record_history(
            &conn,
            bug_id,
            admin.id,
            "status",
            "unparseable_old",
            "unparseable_new",
            0,
        )
        .unwrap();

        // Build email body with current_note Some(...)
        let note_ctx = Some((42, "admin_user", 1700000000, "Note test text"));
        db.build_mantis_email_body(
            &conn,
            bug_id,
            "The following issue has been TESTED.",
            note_ctx,
        )
        .unwrap()
    };

    assert!(body.contains("The following issue has been TESTED."));
    assert!(body.contains("Steps to Reproduce:\n1. Step A\n2. Step B"));
    assert!(body.contains("Additional Information:\nExtra system info"));
    assert!(body.contains("(0000042) admin_user"));
    assert!(body.contains("Note test text"));
    assert!(body.contains("File Added: attached_file.txt"));
    assert!(body.contains("File Deleted: old_file.txt"));
    assert!(body.contains("Priority"));
    assert!(body.contains("Severity"));
    assert!(body.contains("Reproducibility"));
    assert!(body.contains("Category"));
    assert!(body.contains("Target Version"));
    assert!(body.contains("Fixed in Version"));
    assert!(body.contains("Product Version"));
    assert!(body.contains("View Status"));
    assert!(body.contains("custom_field_x"));
    assert!(body.contains("user#9998 => user#9999"));
    assert!(body.contains("unparseable_old => unparseable_new"));

    // 6. Test recipients edge cases in notify_bug_event_filtered:
    // - disabled user
    let disabled_u = db
        .create_user(
            "disabled_user",
            "",
            "dis@example.com",
            "pw",
            55,
            false,
            false,
        )
        .unwrap();
    db.add_bug_monitor(bug_id, disabled_u).unwrap();

    // - user with empty email
    let empty_email_u = db
        .create_user("empty_email", "", "   ", "pw", 55, true, false)
        .unwrap();
    db.add_bug_monitor(bug_id, empty_email_u).unwrap();

    // - extra_recipients with <= 0 and non-existent id
    db.flush_email_queue().unwrap();
    {
        let conn = db.conn();
        db.notify_bug_event_filtered(
            &conn,
            bug_id,
            0,
            &[-1, 0, 99999, admin.id], // -1 and 0 skipped, 99999 non-existent, admin.id deduplicated
            "Edge Test Subject",
            &body,
        );
    }
    let emails = db.list_queued_emails().unwrap();
    let recs: Vec<&str> = emails.iter().map(|e| e.email.as_str()).collect();
    // admin receives it, disabled_user and empty_email do NOT
    assert!(!recs.contains(&"dis@example.com"));
    assert!(!recs.contains(&""));
    assert_eq!(
        recs.iter().filter(|&&e| e == admin.email.as_str()).count(),
        1
    );

    // 7. Test delete_queued_email
    let email_id = emails[0].email_id;
    db.delete_queued_email(email_id).unwrap();
    let remaining = db.list_queued_emails().unwrap();
    assert_eq!(remaining.len(), 0);
}

#[test]
fn test_db_bug_update_fields_and_history() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let pid = db
        .create_project("BugUpdateProj", 10, 10, "Test project", true)
        .unwrap();

    let bug_id = db
        .create_bug(
            pid,
            admin.id,
            0,
            "Initial summary",
            "Initial description",
            "",
            "",
            30,
            50,
            70,
            "1.0",
            "Linux",
            "6.8",
            "x86_64",
            10,
        )
        .unwrap();

    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.build, "");
    assert_eq!(bug.due_date, 0);
    assert_eq!(bug.sticky, 0);
    assert_eq!(bug.os, "Linux");
    assert_eq!(bug.os_build, "6.8");
    assert_eq!(bug.platform, "x86_64");

    // Update build, due_date, sticky, os, os_build, platform
    let update = BugUpdate {
        os: Some("macOS".to_string()),
        os_build: Some("14.5".to_string()),
        platform: Some("arm64".to_string()),
        build: Some("build-101".to_string()),
        due_date: Some(1750000000),
        sticky: Some(1),
        ..Default::default()
    };

    let changes = db.update_bug_fields(bug_id, admin.id, update).unwrap();
    assert_eq!(changes.len(), 6);

    let updated = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(updated.os, "macOS");
    assert_eq!(updated.os_build, "14.5");
    assert_eq!(updated.platform, "arm64");
    assert_eq!(updated.build, "build-101");
    assert_eq!(updated.due_date, 1750000000);
    assert_eq!(updated.sticky, 1);

    // Verify history records
    let history = db.list_bug_history(bug_id).unwrap();
    let field_names: Vec<&str> = history.iter().map(|h| h.field_name.as_str()).collect();
    assert!(field_names.contains(&"os"));
    assert!(field_names.contains(&"os_build"));
    assert!(field_names.contains(&"platform"));
    assert!(field_names.contains(&"build"));
    assert!(field_names.contains(&"due_date"));
    assert!(field_names.contains(&"sticky"));

    let sticky_hist = history.iter().find(|h| h.field_name == "sticky").unwrap();
    assert_eq!(sticky_hist.old_value, "0");
    assert_eq!(sticky_hist.new_value, "1");

    let due_hist = history.iter().find(|h| h.field_name == "due_date").unwrap();
    assert_eq!(due_hist.old_value, "0");
    assert_eq!(due_hist.new_value, "1750000000");
}

#[test]
fn test_db_category_default_assignee() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let pid = db
        .create_project("CatAssignProj", 10, 10, "Test project", true)
        .unwrap();

    // 1. Create developer user (access 55)
    let dev_id = db
        .create_user("dev1", "Dev One", "dev1@example.com", "pw", 55, true, false)
        .unwrap();

    // 2. Create category with user_id = dev_id
    let cat_id = db.add_category(pid, "Backend", dev_id).unwrap();

    // 3. Create bug with this category -> auto assigned to dev_id with status 50
    let bug_id = db
        .create_bug(
            pid,
            admin.id,
            cat_id,
            "Auto assigned bug",
            "Description",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(bug.handler_id, dev_id);
    assert_eq!(bug.status, 50); // Assigned

    // 4. Category with reporter user (access 25 < 55) -> not auto-assigned
    let rep_id = db
        .create_user(
            "rep1",
            "Reporter One",
            "rep1@example.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    let cat2_id = db.add_category(pid, "Frontend", rep_id).unwrap();
    let bug2_id = db
        .create_bug(
            pid,
            admin.id,
            cat2_id,
            "Unassigned bug (reporter)",
            "Desc",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let bug2 = db.get_bug(bug2_id).unwrap().unwrap();
    assert_eq!(bug2.handler_id, 0);
    assert_eq!(bug2.status, 10); // New

    // 5. Category with disabled user (enabled = 0) -> not auto-assigned
    let dis_dev_id = db
        .create_user(
            "dev_dis",
            "Disabled Dev",
            "dis@example.com",
            "pw",
            55,
            false,
            false,
        )
        .unwrap();
    let cat3_id = db.add_category(pid, "DevOps", dis_dev_id).unwrap();
    let bug3_id = db
        .create_bug(
            pid,
            admin.id,
            cat3_id,
            "Unassigned bug (disabled)",
            "Desc",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let bug3 = db.get_bug(bug3_id).unwrap().unwrap();
    assert_eq!(bug3.handler_id, 0);
    assert_eq!(bug3.status, 10);
}

#[test]
fn test_db_project_access_min_enforcement() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();

    // Create public project (view_state = 10) with access_min = 40 (Updater)
    let pid = db
        .create_project("RestrictedPublic", 10, 10, "Min access 40", true)
        .unwrap();
    db.set_project_access_min(pid, 40).unwrap();

    // Create user with access_level 25 (Reporter)
    let rep_id = db
        .create_user("rep_user", "", "rep@example.com", "pw", 25, true, false)
        .unwrap();
    let rep = db.get_user_by_id(rep_id).unwrap().unwrap();

    // Create user with access_level 40 (Updater)
    let upd_id = db
        .create_user("upd_user", "", "upd@example.com", "pw", 40, true, false)
        .unwrap();
    let upd = db.get_user_by_id(upd_id).unwrap().unwrap();

    // Reporter (< access_min) gets effective access 0
    assert_eq!(db.effective_access_level(&rep, pid).unwrap(), 0);

    // Updater (>= access_min) gets effective access 40
    assert_eq!(db.effective_access_level(&upd, pid).unwrap(), 40);

    // Admin always gets 90
    assert_eq!(db.effective_access_level(&admin, pid).unwrap(), 90);

    // Explicit override in mantis_project_user_list_table grants access to Reporter
    db.set_project_user(pid, rep_id, 55).unwrap();
    assert_eq!(db.effective_access_level(&rep, pid).unwrap(), 55);
}

#[test]
fn test_db_subproject_descendants_and_bug_rollup() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();

    // Root project A
    let proj_a = db.create_project("ProjectA", 10, 10, "Root", true).unwrap();
    // Subproject B (child of A)
    let proj_b = db
        .create_project("ProjectB", 10, 10, "Child of A", true)
        .unwrap();
    // Subproject C (child of B)
    let proj_c = db
        .create_project("ProjectC", 10, 10, "Child of B", true)
        .unwrap();

    db.add_subproject(proj_a, proj_b, true).unwrap();
    db.add_subproject(proj_b, proj_c, true).unwrap();

    let descendants = db.list_project_descendant_ids(proj_a).unwrap();
    assert_eq!(descendants, vec![proj_a, proj_b, proj_c]);

    // Create bugs in A, B, C
    let bug_a = db
        .create_bug(
            proj_a, admin.id, 0, "Bug in A", "Desc A", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();
    let bug_b = db
        .create_bug(
            proj_b, admin.id, 0, "Bug in B", "Desc B", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();
    let bug_c = db
        .create_bug(
            proj_c, admin.id, 0, "Bug in C", "Desc C", "", "", 30, 50, 70, "", "", "", "", 10,
        )
        .unwrap();

    // Filter bugs on project A -> rolls up bugs from A, B, and C
    let filter_a = BugFilter {
        project_id: Some(proj_a),
        ..Default::default()
    };
    let results = db.filter_bugs(&filter_a, &admin).unwrap();
    let result_ids: Vec<i64> = results.iter().map(|b| b.id).collect();
    assert!(result_ids.contains(&bug_a));
    assert!(result_ids.contains(&bug_b));
    assert!(result_ids.contains(&bug_c));

    // Create private subproject D (child of A, view_state = 50)
    let proj_d = db
        .create_project("ProjectD_Private", 10, 50, "Private child of A", true)
        .unwrap();
    db.add_subproject(proj_a, proj_d, true).unwrap();
    let bug_d = db
        .create_bug(
            proj_d,
            admin.id,
            0,
            "Bug in D Private",
            "Desc D",
            "",
            "",
            30,
            50,
            70,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Reporter user with no access to private project D
    let rep_id = db
        .create_user(
            "rep_rollup",
            "",
            "rep_rollup@example.com",
            "pw",
            25,
            true,
            false,
        )
        .unwrap();
    let rep = db.get_user_by_id(rep_id).unwrap().unwrap();

    let rep_results = db.filter_bugs(&filter_a, &rep).unwrap();
    let rep_result_ids: Vec<i64> = rep_results.iter().map(|b| b.id).collect();
    assert!(rep_result_ids.contains(&bug_a));
    assert!(rep_result_ids.contains(&bug_b));
    assert!(rep_result_ids.contains(&bug_c));
    assert!(!rep_result_ids.contains(&bug_d)); // Private subproject isolated!
}

#[test]
fn test_tag_usage_counts_and_helpers() {
    let db = Db::open_in_memory().unwrap();
    let uid = db
        .create_user("tagadmin", "Admin", "a@t.com", "pass", 90, true, false)
        .unwrap();
    let pid = db.create_project("TagProj", 10, 10, "", true).unwrap();
    let b1 = db
        .create_bug(
            pid, uid, 1, "Bug 1", "Desc", "", "", 10, 10, 10, "", "", "", "", 10,
        )
        .unwrap();
    let b2 = db
        .create_bug(
            pid, uid, 1, "Bug 2", "Desc", "", "", 10, 10, 10, "", "", "", "", 10,
        )
        .unwrap();

    let t1 = db.create_tag(uid, "alpha", "Alpha tag").unwrap();
    let t2 = db.create_tag(uid, "beta", "Beta tag").unwrap();

    db.attach_tag_to_bug(b1, t1, uid).unwrap();
    db.attach_tag_to_bug(b2, t1, uid).unwrap();

    let counts = db.list_tags_with_counts().unwrap();
    assert_eq!(counts.len(), 2);
    let alpha = counts.iter().find(|(t, _)| t.name == "alpha").unwrap();
    assert_eq!(alpha.1, 2);
    let beta = counts.iter().find(|(t, _)| t.name == "beta").unwrap();
    assert_eq!(beta.1, 0);

    let (t1_fetched, t1_count) = db.get_tag_with_count_by_id(t1).unwrap().unwrap();
    assert_eq!(t1_fetched.name, "alpha");
    assert_eq!(t1_count, 2);

    let (t2_fetched, t2_count) = db.get_tag_with_count_by_id(t2).unwrap().unwrap();
    assert_eq!(t2_fetched.name, "beta");
    assert_eq!(t2_count, 0);

    assert!(db.get_tag_with_count_by_id(9999).unwrap().is_none());
}

#[test]
fn test_custom_field_regex_validation_in_db() {
    let db = Db::open_in_memory().unwrap();
    let pid = db.create_project("RegexpProj", 10, 10, "", true).unwrap();
    let uid = db
        .create_user(
            "admin_reg",
            "Admin Reg",
            "ar@example.com",
            "pass",
            90,
            true,
            false,
        )
        .unwrap();

    // 1. Invalid regex rejected on create_custom_field_full
    let create_err = db
        .create_custom_field_full(
            "InvalidCF",
            0,
            "",
            "",
            "[unclosed-regex",
            10,
            25,
            0,
            0,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap_err();
    assert!(
        create_err.contains("Invalid regular expression"),
        "Expected invalid regex error on create, got: {}",
        create_err
    );

    // 2. Valid regex succeeds on create
    let cf_id = db
        .create_custom_field_full(
            "ProductSKU",
            0,
            "",
            "",
            r"^[0-9]{4}-[A-Z]{2}$",
            10,
            25,
            0,
            0,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap();
    db.link_custom_field_to_project(cf_id, pid, 1).unwrap();

    // 3. Invalid regex rejected on update_custom_field_full
    let update_err = db
        .update_custom_field_full(
            cf_id,
            "ProductSKU",
            0,
            "",
            "",
            "(?P<broken",
            10,
            25,
            0,
            0,
            false,
            false,
            true,
            true,
            false,
            false,
            false,
            false,
        )
        .unwrap_err();
    assert!(
        update_err.contains("Invalid regular expression"),
        "Expected invalid regex error on update, got: {}",
        update_err
    );

    // 4. Create bug and verify set_custom_field_value regex enforcement
    let bid = db
        .create_bug(
            pid,
            uid,
            1,
            "SKU Bug",
            "Testing SKU regex",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Non-matching value fails
    let val_err = db
        .set_custom_field_value(cf_id, bid, uid, "123-abc")
        .unwrap_err();
    assert!(
        val_err.contains("does not match required pattern"),
        "Expected pattern mismatch, got: {}",
        val_err
    );

    // Matching value succeeds
    db.set_custom_field_value(cf_id, bid, uid, "2024-XY")
        .unwrap();
    let cf_vals = db.get_custom_field_values_for_bug(bid).unwrap();
    assert_eq!(cf_vals.len(), 1);
    assert_eq!(cf_vals[0].value, "2024-XY");

    // Empty value clears field without regex error
    db.set_custom_field_value(cf_id, bid, uid, "").unwrap();
    let cf_vals_cleared = db.get_custom_field_values_for_bug(bid).unwrap();
    assert_eq!(cf_vals_cleared.len(), 1);
    assert_eq!(cf_vals_cleared[0].value, "");
}

#[test]
fn test_revisions_and_bugnote_edits_db() {
    let db = Db::open_in_memory().unwrap();
    // Test idempotent migration
    assert!(db.migrate().is_ok());

    let uid = db
        .create_user("revuser", "Rev User", "r@u.com", "pass", 90, true, false)
        .unwrap();
    let pid = db.create_project("RevProj", 10, 10, "", true).unwrap();
    let bug_id = db
        .create_bug(
            pid,
            uid,
            1,
            "Rev Bug",
            "Initial Desc",
            "Initial Steps",
            "Initial AddInfo",
            10,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // 1. Verify no revisions initially
    let revs0 = db.list_bug_revisions(bug_id, None).unwrap();
    assert!(revs0.is_empty());

    // 2. Update description, steps, and additional_info
    let update_fields = BugUpdate {
        description: Some("Updated Desc".to_string()),
        steps_to_reproduce: Some("Updated Steps".to_string()),
        additional_information: Some("Updated AddInfo".to_string()),
        ..Default::default()
    };
    let hist = db.update_bug_fields(bug_id, uid, update_fields).unwrap();
    assert_eq!(hist.len(), 3);

    // Verify 3 revisions created with previous values
    let revs = db.list_bug_revisions(bug_id, None).unwrap();
    assert_eq!(revs.len(), 3);

    let desc_rev = revs
        .iter()
        .find(|r| r.rev_type == RevisionType::Description as i32)
        .unwrap();
    assert_eq!(desc_rev.value, "Initial Desc");
    assert_eq!(desc_rev.bugnote_id, 0);
    assert_eq!(desc_rev.bug_id, bug_id);

    let steps_rev = revs
        .iter()
        .find(|r| r.rev_type == RevisionType::StepsToReproduce as i32)
        .unwrap();
    assert_eq!(steps_rev.value, "Initial Steps");
    assert_eq!(steps_rev.bugnote_id, 0);

    let info_rev = revs
        .iter()
        .find(|r| r.rev_type == RevisionType::AdditionalInfo as i32)
        .unwrap();
    assert_eq!(info_rev.value, "Initial AddInfo");
    assert_eq!(info_rev.bugnote_id, 0);

    // 3. Test get_bug_revision_by_id
    let fetched = db.get_bug_revision_by_id(desc_rev.id).unwrap().unwrap();
    assert_eq!(fetched.id, desc_rev.id);
    assert_eq!(fetched.value, "Initial Desc");

    // 4. Test bugnote edits and revision
    let note_id = db.add_bugnote(bug_id, uid, "Initial Note", 10).unwrap();

    // Reject empty note edit
    assert!(db.update_bugnote(note_id, uid, "   ").is_err());

    // Valid note edit
    assert!(db.update_bugnote(note_id, uid, "Updated Note").is_ok());

    let updated_note = db.get_bugnote_by_id(note_id).unwrap().unwrap();
    assert_eq!(updated_note.note, "Updated Note");

    // Check bugnote revisions via list_bug_revisions with bugnote_id filter
    let note_revs = db.list_bug_revisions(bug_id, Some(note_id)).unwrap();
    assert_eq!(note_revs.len(), 1);
    assert_eq!(note_revs[0].bugnote_id, note_id);
    assert_eq!(note_revs[0].rev_type, RevisionType::Bugnote as i32);
    assert_eq!(note_revs[0].value, "Initial Note");

    // Total revisions for bug is now 4
    let all_revs = db.list_bug_revisions(bug_id, None).unwrap();
    assert_eq!(all_revs.len(), 4);

    // 5. Test set_bugnote_view_state
    assert!(db.set_bugnote_view_state(note_id, uid, 99).is_err()); // invalid
    assert!(db.set_bugnote_view_state(note_id, uid, 50).is_ok()); // private
    let note_priv = db.get_bugnote_by_id(note_id).unwrap().unwrap();
    assert_eq!(note_priv.view_state, 50);

    assert!(db.set_bugnote_view_state(note_id, uid, 10).is_ok()); // public
    let note_pub = db.get_bugnote_by_id(note_id).unwrap().unwrap();
    assert_eq!(note_pub.view_state, 10);

    // 6. Test drop_bug_revision
    assert!(db.drop_bug_revision(desc_rev.id).is_ok());
    assert!(db.get_bug_revision_by_id(desc_rev.id).unwrap().is_none());
    let revs_after_drop = db.list_bug_revisions(bug_id, None).unwrap();
    assert_eq!(revs_after_drop.len(), 3);

    // 7. Test delete_bugnote cleans up revisions for that note
    assert!(db.delete_bugnote(note_id).is_ok());
    let note_revs_after_delete = db.list_bug_revisions(bug_id, Some(note_id)).unwrap();
    assert!(note_revs_after_delete.is_empty());

    // 8. Test optimistic locking via expected_last_updated in update_bug_fields
    let cur_bug = db.get_bug(bug_id).unwrap().unwrap();
    let stale_update = BugUpdate {
        summary: Some("Stale update".to_string()),
        expected_last_updated: Some(cur_bug.last_updated - 100),
        ..Default::default()
    };
    let err = db.update_bug_fields(bug_id, uid, stale_update).unwrap_err();
    assert!(err.starts_with("CONFLICT:"));

    let valid_update = BugUpdate {
        summary: Some("Valid update".to_string()),
        expected_last_updated: Some(cur_bug.last_updated),
        ..Default::default()
    };
    assert!(db.update_bug_fields(bug_id, uid, valid_update).is_ok());

    // 9. Test delete_bug cleans up all remaining revisions
    assert!(db.delete_bug(bug_id).is_ok());
    let revs_after_del = db.list_bug_revisions(bug_id, None).unwrap();
    assert!(revs_after_del.is_empty());
}

#[test]
fn test_clone_bug_comprehensive() {
    let db = Db::open_in_memory().unwrap();
    let p1 = db.create_project("Project 1", 10, 10, "", true).unwrap();
    let p2 = db.create_project("Project 2", 10, 10, "", false).unwrap();
    let u1 = db
        .create_user("rep1", "Reporter 1", "r1@test.com", "pw", 25, true, false)
        .unwrap();
    let u2 = db
        .create_user("rep2", "Reporter 2", "r2@test.com", "pw", 25, true, false)
        .unwrap();

    // Create custom category in p1 and p2
    let c1 = db.add_category(p1, "P1 Special", 0).unwrap();
    let c2 = db.add_category(p2, "P2 Special", 0).unwrap();

    // Create custom field linked to p1 and p2
    let cf1 = db
        .create_custom_field("Shared CF", 0, "", "", "", 0, 100, false, false, true, true)
        .unwrap();
    db.link_custom_field_to_project(cf1, p1, 1).unwrap();
    db.link_custom_field_to_project(cf1, p2, 1).unwrap();

    // Custom field only linked to p1
    let cf2 = db
        .create_custom_field(
            "P1 Only CF",
            0,
            "",
            "",
            "",
            0,
            100,
            false,
            false,
            true,
            true,
        )
        .unwrap();
    db.link_custom_field_to_project(cf2, p1, 2).unwrap();

    // Create source bug in p1
    let source_id = db
        .create_bug(
            p1,
            u1,
            c1,
            "Original Summary",
            "Detailed Description",
            "Step 1, Step 2",
            "Additional Info",
            40,
            60,
            20,
            "1.0.0",
            "macOS",
            "23A344",
            "arm64",
            10,
        )
        .unwrap();

    // Set additional fields on source bug
    db.update_bug_fields(
        source_id,
        u1,
        BugUpdate {
            target_version: Some("1.1.0".to_string()),
            fixed_in_version: Some("1.0.1".to_string()),
            build: Some("Build-99".to_string()),
            due_date: Some(1700000000),
            ..Default::default()
        },
    )
    .unwrap();

    // Set custom field values on source bug
    db.set_custom_field_value(cf1, source_id, u1, "Shared Value")
        .unwrap();
    db.set_custom_field_value(cf2, source_id, u1, "P1 Exclusive")
        .unwrap();

    // Add tags to source bug
    let tag1 = db.create_tag(u1, "v2-feature", "").unwrap();
    db.attach_tag_to_bug(source_id, tag1, u1).unwrap();

    // Add relationships to source bug
    let other_bug = db
        .create_bug(
            p1,
            u1,
            c1,
            "Other Bug",
            "",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.add_bug_relationship(source_id, other_bug, 0).unwrap(); // RelatedTo

    let dup_bug = db
        .create_bug(
            p1,
            u1,
            c1,
            "Duplicate Bug",
            "",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    // dup_bug is duplicate of source_id
    db.add_bug_relationship(dup_bug, source_id, 1).unwrap();

    // Add attachments to source bug: 1 blob and 1 disk file
    let blob_file_id = db
        .add_bug_file(
            source_id,
            u1,
            "blob.txt",
            "text/plain",
            b"blob data".to_vec(),
            "",
        )
        .unwrap();
    assert!(blob_file_id > 0);

    let temp_dir =
        std::env::temp_dir().join(format!("mantis_test_upload_{}", rand::random::<u64>()));
    let disk_file_id = db
        .add_bug_file(
            source_id,
            u1,
            "disk.txt",
            "text/plain",
            b"disk file content".to_vec(),
            temp_dir.to_str().unwrap(),
        )
        .unwrap();
    assert!(disk_file_id > 0);

    // 1. Clone within same project (p1)
    let cloned_same = db
        .clone_bug(source_id, u2, p1, true, true, temp_dir.to_str().unwrap())
        .unwrap();
    assert_ne!(source_id, cloned_same);

    let full_clone = db.get_bug_full(cloned_same).unwrap().unwrap();
    assert_eq!(full_clone.bug.project_id, p1);
    assert_eq!(full_clone.bug.reporter_id, u2);
    assert_eq!(full_clone.bug.category_id, c1);
    assert_eq!(full_clone.bug.summary, "Original Summary");
    assert_eq!(full_clone.description, "Detailed Description");
    assert_eq!(full_clone.steps_to_reproduce, "Step 1, Step 2");
    assert_eq!(full_clone.additional_information, "Additional Info");
    assert_eq!(full_clone.bug.priority, 40);
    assert_eq!(full_clone.bug.severity, 60);
    assert_eq!(full_clone.bug.reproducibility, 20);
    assert_eq!(full_clone.bug.os, "macOS");
    assert_eq!(full_clone.bug.os_build, "23A344");
    assert_eq!(full_clone.bug.platform, "arm64");
    assert_eq!(full_clone.bug.version, "1.0.0");
    assert_eq!(full_clone.bug.target_version, "1.1.0");
    assert_eq!(full_clone.bug.fixed_in_version, "1.0.1");
    assert_eq!(full_clone.bug.build, "Build-99");
    assert_eq!(full_clone.bug.due_date, 1700000000);
    assert_eq!(full_clone.bug.view_state, 10);
    assert_eq!(full_clone.bug.status, 10);

    // Verify custom fields copied (both cf1 and cf2 exist in p1)
    let cf_vals_same = db.get_custom_field_values_for_bug(cloned_same).unwrap();
    assert!(cf_vals_same
        .iter()
        .any(|v| v.field_id == cf1 && v.value == "Shared Value"));
    assert!(cf_vals_same
        .iter()
        .any(|v| v.field_id == cf2 && v.value == "P1 Exclusive"));

    // Verify tags copied
    let tags_same = db.list_bug_tags(cloned_same).unwrap();
    assert!(tags_same.iter().any(|t| t.tag_id == tag1));

    // Verify relationships: RelatedTo source_id + copied relationship to other_bug
    let rels_same = db.list_bug_relationships(cloned_same).unwrap();
    assert!(rels_same
        .iter()
        .any(|r| r.destination_bug_id == source_id && r.relationship_type == 0));
    assert!(rels_same
        .iter()
        .any(|r| r.destination_bug_id == other_bug && r.relationship_type == 0));

    // Verify third issue (other_bug) is completely unchanged:
    // It has no relationship row pointing at cloned_same, and its duplicate_id is 0
    let other_rels = db.list_bug_relationships(other_bug).unwrap();
    assert!(!other_rels
        .iter()
        .any(|r| r.destination_bug_id == cloned_same || r.source_bug_id == cloned_same));
    let other_bug_full = db.get_bug(other_bug).unwrap().unwrap();
    assert_eq!(other_bug_full.duplicate_id, 0);

    // Verify dup_bug relationships and duplicate_id are completely unchanged:
    // It has only 1 relationship pointing to source_id, not cloned_same
    let dup_rels = db.list_bug_relationships(dup_bug).unwrap();
    assert_eq!(dup_rels.len(), 1);
    assert_eq!(dup_rels[0].destination_bug_id, source_id);
    assert_eq!(dup_rels[0].relationship_type, 1);
    let dup_bug_full = db.get_bug(dup_bug).unwrap().unwrap();
    assert_eq!(dup_bug_full.duplicate_id, source_id);

    // Verify attachments copied separately
    let files_same = db.list_bug_files(cloned_same).unwrap();
    assert_eq!(files_same.len(), 2);
    let disk_clone_file = files_same
        .iter()
        .find(|f| f.filename == "disk.txt")
        .unwrap();
    let src_disk_file = db.get_bug_file_meta(disk_file_id).unwrap().unwrap();
    assert_ne!(disk_clone_file.diskfile, src_disk_file.diskfile);

    // Deleting file on cloned bug does NOT affect source bug file!
    db.delete_bug_file(disk_clone_file.id, temp_dir.to_str().unwrap())
        .unwrap();
    assert!(db.get_bug_file_meta(disk_clone_file.id).unwrap().is_none());
    assert!(db.get_bug_file_meta(disk_file_id).unwrap().is_some());
    let src_content = db
        .get_bug_file(disk_file_id, temp_dir.to_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(src_content.1, b"disk file content");

    // 2. Clone to different project (p2):
    // Category c1 is NOT effective in p2, should fall back to c2
    // cf2 is NOT linked to p2, so only cf1 should be copied
    let cloned_p2 = db
        .clone_bug(source_id, u2, p2, false, false, temp_dir.to_str().unwrap())
        .unwrap();
    let full_p2 = db.get_bug_full(cloned_p2).unwrap().unwrap();
    assert_eq!(full_p2.bug.project_id, p2);
    assert_eq!(full_p2.bug.category_id, c2); // fell back to c2!

    let cf_vals_p2 = db.get_custom_field_values_for_bug(cloned_p2).unwrap();
    assert!(cf_vals_p2
        .iter()
        .any(|v| v.field_id == cf1 && v.value == "Shared Value"));
    assert!(!cf_vals_p2.iter().any(|v| v.field_id == cf2)); // cf2 not linked to p2

    // Since copy_relationships = false, only the automatic RelatedTo source is present
    let rels_p2 = db.list_bug_relationships(cloned_p2).unwrap();
    assert_eq!(rels_p2.len(), 1);
    assert_eq!(rels_p2[0].destination_bug_id, source_id);
    assert_eq!(rels_p2[0].relationship_type, 0);

    // History check
    let hist_new = db.list_bug_history(cloned_p2).unwrap();
    assert!(hist_new.iter().any(|h| h.type_ == 1)); // new bug
    assert!(hist_new
        .iter()
        .any(|h| h.type_ == 21 && h.new_value == source_id.to_string())); // created_from

    let hist_src = db.list_bug_history(source_id).unwrap();
    assert!(hist_src
        .iter()
        .any(|h| h.type_ == 20 && h.new_value == cloned_p2.to_string())); // cloned_to

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_move_bug_success_and_category_fallback() {
    let db = Db::open_in_memory().unwrap();
    let p1 = db.create_project("Source Proj", 10, 10, "", true).unwrap();
    let p2 = db.create_project("Target Proj", 10, 10, "", false).unwrap();
    let p_disabled = db
        .create_project("Disabled Proj", 10, 10, "", true)
        .unwrap();
    db.update_project(p_disabled, "Disabled Proj", 10, false, 10, "", true)
        .unwrap();
    let u = db
        .create_user("dev", "Developer", "d@test.com", "pw", 55, true, false)
        .unwrap();

    let c1 = db.add_category(p1, "Source Category", 0).unwrap();
    let c2 = db.add_category(p2, "Target Category", 0).unwrap();

    let bug_id = db
        .create_bug(
            p1,
            u,
            c1,
            "Move Test Bug",
            "Description",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Fails on non-existent project
    assert!(db.move_bug(bug_id, u, 9999).is_err());
    // Fails on disabled project
    assert!(db.move_bug(bug_id, u, p_disabled).is_err());

    // Move to p2: c1 is not valid in p2 -> falls back to c2
    db.move_bug(bug_id, u, p2).unwrap();

    let moved_bug = db.get_bug(bug_id).unwrap().unwrap();
    assert_eq!(moved_bug.project_id, p2);
    assert_eq!(moved_bug.category_id, c2);

    let history = db.list_bug_history(bug_id).unwrap();
    assert!(history.iter().any(|h| h.field_name == "project_id"
        && h.old_value == p1.to_string()
        && h.new_value == p2.to_string()));
    assert!(history.iter().any(|h| h.field_name == "category_id"
        && h.old_value == c1.to_string()
        && h.new_value == c2.to_string()));

    // Move when already in target project is a no-op Ok(())
    assert!(db.move_bug(bug_id, u, p2).is_ok());
}

#[test]
fn test_clone_bug_outgoing_duplicate_relationship() {
    let db = Db::open_in_memory().unwrap();
    let p = db.create_project("Test Proj", 10, 10, "", true).unwrap();
    let u = db
        .create_user("dev", "Dev", "d@test.com", "pw", 55, true, false)
        .unwrap();

    let parent_bug = db
        .create_bug(
            p,
            u,
            1,
            "Parent Bug",
            "",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let dup_source = db
        .create_bug(
            p,
            u,
            1,
            "Dup Source",
            "",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // dup_source is duplicate_of parent_bug
    db.add_bug_relationship(dup_source, parent_bug, 1).unwrap();
    assert_eq!(
        db.get_bug(dup_source).unwrap().unwrap().duplicate_id,
        parent_bug
    );

    // Clone dup_source
    let cloned = db.clone_bug(dup_source, u, p, true, false, "").unwrap();

    // Verify cloned issue inherits duplicate_of relationship to parent_bug (matching MantisBT relationship_copy_all)
    let cloned_bug = db.get_bug(cloned).unwrap().unwrap();
    assert_eq!(cloned_bug.duplicate_id, parent_bug);

    let cloned_rels = db.list_bug_relationships(cloned).unwrap();
    assert!(cloned_rels
        .iter()
        .any(|r| r.destination_bug_id == parent_bug && r.relationship_type == 1));

    // Verify parent_bug has reciprocal has_duplicate relationship pointing to cloned
    let parent_rels = db.list_bug_relationships(parent_bug).unwrap();
    assert!(parent_rels
        .iter()
        .any(|r| r.destination_bug_id == cloned && r.relationship_type == 2));
}

#[test]
fn test_clone_bug_cleanup_guard_removes_partial_writes_on_failure() {
    let db = Db::open_in_memory().unwrap();
    let p = db
        .create_project("Disk Cleanup Proj", 10, 10, "", true)
        .unwrap();
    let u = db
        .create_user("dev_disk", "Dev Disk", "dd@test.com", "pw", 55, true, false)
        .unwrap();

    let temp_dir = std::env::temp_dir().join(format!(
        "mantis_test_cleanup_guard_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp_dir_str = temp_dir.to_str().unwrap();

    let bug_id = db
        .create_bug(
            p,
            u,
            0,
            "Bug with disk attachments",
            "desc",
            "",
            "",
            50,
            10,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Add first attachment with file on disk
    let file1_bytes = b"First attachment content";
    let _f1_id = db
        .add_bug_file(
            bug_id,
            u,
            "file1.txt",
            "text/plain",
            file1_bytes.to_vec(),
            temp_dir_str,
        )
        .unwrap();

    // Add second attachment with file on disk
    let file2_bytes = b"Second attachment content";
    let _f2_id = db
        .add_bug_file(
            bug_id,
            u,
            "file2.txt",
            "text/plain",
            file2_bytes.to_vec(),
            temp_dir_str,
        )
        .unwrap();

    // Now find the disk path of attachment 2 and delete it from disk to trigger read failure during clone
    let files = db.list_bug_files(bug_id).unwrap();
    assert_eq!(files.len(), 2);
    let disk_file1 = temp_dir.join(&files[0].diskfile);
    let disk_file2 = temp_dir.join(&files[1].diskfile);
    assert!(disk_file2.exists());
    std::fs::remove_file(&disk_file2).unwrap();
    assert!(!disk_file2.exists());

    // Call clone_bug with copy_files = true
    let clone_res = db.clone_bug(bug_id, u, p, false, true, temp_dir_str);
    assert!(
        clone_res.is_err(),
        "Clone should fail due to missing attachment file on disk"
    );

    // Verify cleanup guard removed the copied file for attachment 1
    // The only remaining file in temp_dir should be the original file1.txt
    let remaining_files: Vec<_> = std::fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(
        remaining_files.len(),
        1,
        "Only original file1 should remain; cloned partial files must be cleaned up"
    );
    assert_eq!(remaining_files[0].path(), disk_file1);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_list_bug_monitor_users_ordering() {
    let db = Db::open_in_memory().unwrap();
    let pid = db
        .create_project("Monitors Project", 10, 10, "", true)
        .unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let bid = db
        .create_bug(
            pid,
            admin.id,
            1,
            "Issue for monitors",
            "Details",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let u_charlie = db
        .create_user(
            "charlie",
            "Charlie User",
            "charlie@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let u_alice = db
        .create_user(
            "alice",
            "Alice User",
            "alice@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let u_bob = db
        .create_user(
            "bob",
            "Bob User",
            "bob@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();

    db.add_bug_monitor(bid, u_charlie).unwrap();
    db.add_bug_monitor(bid, u_alice).unwrap();
    db.add_bug_monitor(bid, u_bob).unwrap();

    let monitors = db.list_bug_monitor_users(bid).unwrap();
    assert_eq!(monitors.len(), 3);
    assert_eq!(monitors[0].username, "alice");
    assert_eq!(monitors[1].username, "bob");
    assert_eq!(monitors[2].username, "charlie");
}

#[test]
fn test_add_and_remove_bug_monitor_with_actor_records_history() {
    let db = Db::open_in_memory().unwrap();
    let pid = db
        .create_project("History Monitor Project", 10, 10, "", true)
        .unwrap();
    let actor_id = db
        .create_user(
            "lead_dev",
            "Lead Dev",
            "lead@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();
    let target_id = db
        .create_user(
            "target_user",
            "Target User",
            "target@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();

    let bid = db
        .create_bug(
            pid,
            actor_id,
            1,
            "History Bug",
            "Details",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // 1. Add monitor with actor
    db.add_bug_monitor_with_actor(bid, target_id, actor_id)
        .unwrap();

    let monitors = db.list_bug_monitor_users(bid).unwrap();
    assert_eq!(monitors.len(), 1);
    assert_eq!(monitors[0].id, target_id);

    // Verify history entry type 12 (monitor_added)
    let history = db.list_bug_history(bid).unwrap();
    let add_entry = history.iter().find(|h| h.field_name == "monitor_added");
    assert!(add_entry.is_some(), "Expected monitor_added history entry");
    let add_entry = add_entry.unwrap();
    assert_eq!(add_entry.user_id, actor_id);
    assert_eq!(add_entry.new_value, "target_user");
    assert_eq!(add_entry.old_value, "");
    assert_eq!(add_entry.type_, 12);

    // 2. Remove monitor with actor
    db.remove_bug_monitor_with_actor(bid, target_id, actor_id)
        .unwrap();

    let monitors_after = db.list_bug_monitor_users(bid).unwrap();
    assert!(monitors_after.is_empty());

    // Verify history entry type 13 (monitor_removed / BUG_UNMONITOR)
    let history_after = db.list_bug_history(bid).unwrap();
    let remove_entry = history_after
        .iter()
        .find(|h| h.field_name == "monitor_removed");
    assert!(
        remove_entry.is_some(),
        "Expected monitor_removed history entry"
    );
    let remove_entry = remove_entry.unwrap();
    assert_eq!(remove_entry.user_id, actor_id);
    assert_eq!(remove_entry.old_value, "target_user");
    assert_eq!(remove_entry.type_, 13);
}

#[test]
fn test_notify_users_only_emails_target_recipients() {
    let db = Db::open_in_memory().unwrap();
    let pid = db
        .create_project("Notify Users Project", 10, 10, "", true)
        .unwrap();

    let reporter_id = db
        .create_user(
            "issue_reporter",
            "Reporter User",
            "reporter@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let handler_id = db
        .create_user(
            "issue_handler",
            "Handler User",
            "handler@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();
    let monitor_id = db
        .create_user(
            "issue_monitor",
            "Monitor User",
            "monitor@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();
    let target_id = db
        .create_user(
            "target_recipient",
            "Target Recipient",
            "target@example.com",
            "password",
            25,
            true,
            false,
        )
        .unwrap();

    let bid = db
        .create_bug(
            pid,
            reporter_id,
            1,
            "Targeted Notification Bug",
            "Details",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Assign handler and add monitor
    db.assign_bug(bid, handler_id, reporter_id).unwrap();
    db.add_bug_monitor(bid, monitor_id).unwrap();

    // Clear any previous queued emails from creation/assignment
    let conn = db.conn();
    conn.execute("DELETE FROM mantis_email_table", []).unwrap();

    // Now call notify_users strictly targeting target_id
    db.notify_users(
        &conn,
        bid,
        &[target_id],
        10,
        "Reminder Subject",
        "Reminder Body",
    );

    // Verify: ONLY target@example.com received an email
    let mut stmt = conn
        .prepare("SELECT email FROM mantis_email_table ORDER BY email ASC")
        .unwrap();
    let emails: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .unwrap()
        .flatten()
        .collect();

    assert_eq!(emails, vec!["target@example.com".to_string()]);
}

#[test]
fn test_add_bugnote_with_type() {
    let db = Db::open_in_memory().unwrap();
    let pid = db
        .create_project("Bugnote Type Project", 10, 10, "", true)
        .unwrap();
    let uid = db
        .create_user(
            "note_user",
            "Note User",
            "note@example.com",
            "password",
            55,
            true,
            false,
        )
        .unwrap();
    let bid = db
        .create_bug(
            pid,
            uid,
            1,
            "Bugnote Type Bug",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let n1 = db
        .add_bugnote_with_type(bid, uid, "Standard Note", 10, 0, 0)
        .unwrap();
    let n2 = db
        .add_bugnote_with_type(bid, uid, "Reminder Note", 10, 0, 1)
        .unwrap();

    let notes = db.list_bugnotes(bid, 90).unwrap();
    assert_eq!(notes.len(), 2);
    let note1 = notes.iter().find(|n| n.id == n1).unwrap();
    let note2 = notes.iter().find(|n| n.id == n2).unwrap();

    assert_eq!(note1.note_type, 0);
    assert_eq!(note2.note_type, 1);
}

#[test]
fn test_get_bugs_by_ids() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("BatchBugProj", 10, 10, "", true).unwrap();
    let uid = db
        .create_user("batch_user", "Batch", "b@x.com", "pw", 25, true, false)
        .unwrap();

    // 1. Empty slice returns empty map
    let empty_res = db.get_bugs_by_ids(&[]).unwrap();
    assert!(empty_res.is_empty());

    // 2. Non-existent IDs return empty map
    let non_existent = db.get_bugs_by_ids(&[9999, 10000]).unwrap();
    assert!(non_existent.is_empty());

    // 3. Create bugs
    let bug1_id = db
        .create_bug(
            proj_id,
            uid,
            1,
            "Bug 1 Summary",
            "Desc 1",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let bug2_id = db
        .create_bug(
            proj_id,
            uid,
            1,
            "Bug 2 Summary",
            "Desc 2",
            "",
            "",
            40,
            60,
            20,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Query mix of valid and non-existent IDs
    let map = db.get_bugs_by_ids(&[bug1_id, bug2_id, 99999]).unwrap();
    assert_eq!(map.len(), 2);
    assert!(map.contains_key(&bug1_id));
    assert!(map.contains_key(&bug2_id));

    let b1 = &map[&bug1_id];
    assert_eq!(b1.id, bug1_id);
    assert_eq!(b1.project_id, proj_id);
    assert_eq!(b1.summary, "Bug 1 Summary");
    assert_eq!(b1.priority, 30);
    assert_eq!(b1.severity, 50);

    let b2 = &map[&bug2_id];
    assert_eq!(b2.id, bug2_id);
    assert_eq!(b2.project_id, proj_id);
    assert_eq!(b2.summary, "Bug 2 Summary");
    assert_eq!(b2.priority, 40);
    assert_eq!(b2.severity, 60);
}

#[test]
fn test_get_bugnotes_by_ids() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("BatchNoteProj", 10, 10, "", true)
        .unwrap();
    let uid = db
        .create_user("note_user", "Note", "n@x.com", "pw", 25, true, false)
        .unwrap();
    let bug_id = db
        .create_bug(
            proj_id, uid, 1, "Note Bug", "Desc", "", "", 30, 50, 10, "", "", "", "", 10,
        )
        .unwrap();

    // 1. Empty slice returns empty map
    assert!(db.get_bugnotes_by_ids(&[]).unwrap().is_empty());

    // 2. Non-existent IDs return empty map
    assert!(db.get_bugnotes_by_ids(&[8888, 9999]).unwrap().is_empty());

    // 3. Create notes
    let n1_id = db.add_bugnote(bug_id, uid, "First test note", 10).unwrap();
    let n2_id = db
        .add_bugnote(bug_id, uid, "Second private note", 50)
        .unwrap();

    let map = db.get_bugnotes_by_ids(&[n1_id, n2_id, 77777]).unwrap();
    assert_eq!(map.len(), 2);
    assert!(map.contains_key(&n1_id));
    assert!(map.contains_key(&n2_id));

    let note1 = &map[&n1_id];
    assert_eq!(note1.id, n1_id);
    assert_eq!(note1.bug_id, bug_id);
    assert_eq!(note1.reporter_id, uid);
    assert_eq!(note1.view_state, 10);
    assert_eq!(note1.note, "First test note");

    let note2 = &map[&n2_id];
    assert_eq!(note2.id, n2_id);
    assert_eq!(note2.bug_id, bug_id);
    assert_eq!(note2.reporter_id, uid);
    assert_eq!(note2.view_state, 50);
    assert_eq!(note2.note, "Second private note");
}

#[test]
fn test_set_plugin_enabled_admin() {
    let db = Db::open_in_memory().unwrap();

    // 1. Non-existent plugin fails
    let err_not_found = db
        .set_plugin_enabled_admin("NoSuchPlugin", false)
        .unwrap_err();
    assert!(err_not_found.contains("Plugin 'NoSuchPlugin' not found"));

    // 2. Create another protected plugin not in TOGGLEABLE_CORE_PLUGINS
    {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_plugin_table (basename, enabled, protected, priority) VALUES ('OtherProtected', 1, 1, 10)",
            [],
        ).unwrap();
    }
    let err_protected = db
        .set_plugin_enabled_admin("OtherProtected", false)
        .unwrap_err();
    assert!(err_protected.contains("Protected plugin 'OtherProtected' cannot be disabled"));

    // 3. MantisCoreFormatting is in TOGGLEABLE_CORE_PLUGINS: can be toggled
    assert!(db.is_plugin_enabled("MantisCoreFormatting"));
    db.set_plugin_enabled_admin("MantisCoreFormatting", false)
        .unwrap();
    assert!(!db.is_plugin_enabled("MantisCoreFormatting"));

    // Re-enable
    db.set_plugin_enabled_admin("MantisCoreFormatting", true)
        .unwrap();
    assert!(db.is_plugin_enabled("MantisCoreFormatting"));
}

#[test]
fn test_notify_bug_mentions() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("MentionProj", 10, 10, "", true).unwrap();
    let actor_id = db
        .create_user("actor", "Actor", "actor@x.com", "pw", 55, true, false)
        .unwrap();
    let _dev_id = db
        .create_user("dev_alice", "Alice", "alice@x.com", "pw", 55, true, false)
        .unwrap();
    let _rep_id = db
        .create_user("rep_bob", "Bob", "bob@x.com", "pw", 25, true, false)
        .unwrap();
    let disabled_id = db
        .create_user(
            "charlie",
            "Charlie",
            "charlie@x.com",
            "pw",
            25,
            false,
            false,
        )
        .unwrap();
    assert!(disabled_id > 0);

    let bug_id = db
        .create_bug(
            proj_id,
            actor_id,
            1,
            "Mention Issue",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Clear any notification emails created during bug creation
    {
        let conn = db.conn();
        conn.execute("DELETE FROM mantis_email_table", []).unwrap();
    }

    // 1. Public mention of dev_alice, rep_bob, actor himself, and non-existent user
    let mentions = vec![
        "dev_alice".to_string(),
        "rep_bob".to_string(),
        "actor".to_string(), // Self mention should be ignored
        "non_existent".to_string(),
        "charlie".to_string(),   // Disabled user should be ignored
        "DEV_ALICE".to_string(), // Case-insensitive duplicate should be deduplicated
    ];

    db.notify_bug_mentions(
        bug_id,
        actor_id,
        &mentions,
        false,
        "Public note referencing team",
    )
    .unwrap();

    let queued = db.list_queued_emails().unwrap();
    assert_eq!(
        queued.len(),
        2,
        "Should only enqueue 2 emails: dev_alice and rep_bob"
    );
    let recipients: Vec<String> = queued.iter().map(|e| e.email.clone()).collect();
    assert!(recipients.contains(&"alice@x.com".to_string()));
    assert!(recipients.contains(&"bob@x.com".to_string()));
    assert!(!recipients.contains(&"actor@x.com".to_string()));
    assert!(!recipients.contains(&"charlie@x.com".to_string()));

    // Verify email subject & body
    let alice_email = queued.iter().find(|e| e.email == "alice@x.com").unwrap();
    assert!(alice_email
        .subject
        .contains("Mentioned by actor: Mention Issue"));
    assert!(alice_email.body.contains("You were mentioned by actor"));
    assert!(alice_email.body.contains("Public note referencing team"));

    // 2. Clear emails and test private note mentions
    // For a private note, reporter (rep_bob with access 25 < 55) should NOT be notified
    {
        let conn = db.conn();
        conn.execute("DELETE FROM mantis_email_table", []).unwrap();
    }

    let private_mentions = vec!["dev_alice".to_string(), "rep_bob".to_string()];
    db.notify_bug_mentions(
        bug_id,
        actor_id,
        &private_mentions,
        true, // is_private_note = true
        "Confidential security note",
    )
    .unwrap();

    let queued_private = db.list_queued_emails().unwrap();
    assert_eq!(
        queued_private.len(),
        1,
        "Only developer with access >= 55 should be notified of private notes"
    );
    assert_eq!(queued_private[0].email, "alice@x.com");
}

#[test]
fn test_add_bugnote_mention_deduplication() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("DedupProj", 10, 10, "", true).unwrap();
    let rep_alice = db
        .create_user("alice", "Alice", "alice@test.com", "pw", 25, true, false)
        .unwrap();
    let handler_bob = db
        .create_user("bob", "Bob", "bob@test.com", "pw", 55, true, false)
        .unwrap();
    let actor = db
        .create_user(
            "charlie",
            "Charlie",
            "charlie@test.com",
            "pw",
            55,
            true,
            false,
        )
        .unwrap();

    let bug_id = db
        .create_bug(
            proj_id,
            rep_alice,
            1,
            "Issue for dedup testing",
            "Description",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Assign bob as handler
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE mantis_bug_table SET handler_id = ?1 WHERE id = ?2",
            rusqlite::params![handler_bob, bug_id],
        )
        .unwrap();
        // Clear creation emails
        conn.execute("DELETE FROM mantis_email_table", []).unwrap();
    }

    // Charlie adds a bugnote mentioning Bob (@bob)
    let note_text = "Please check this @bob";
    let mentions = vec!["bob".to_string()];

    // Add note with mentions excluded from standard note notification
    db.add_bugnote_with_time_and_mentions(bug_id, actor, note_text, 10, 0, &mentions)
        .unwrap();

    // Notify mentions
    db.notify_bug_mentions(bug_id, actor, &mentions, false, note_text)
        .unwrap();

    let queued = db.list_queued_emails().unwrap();
    // Alice (reporter) gets "Note added by charlie"
    // Bob (handler, mentioned) gets "Mentioned by charlie"
    // Bob MUST NOT receive a second duplicate "Note added" email!
    assert_eq!(
        queued.len(),
        2,
        "Total queued emails must be exactly 2 (no duplicate email for mentioned handler)"
    );

    let bob_emails: Vec<_> = queued
        .iter()
        .filter(|e| e.email == "bob@test.com")
        .collect();
    assert_eq!(
        bob_emails.len(),
        1,
        "Mentioned handler bob should receive exactly 1 email"
    );
    assert!(
        bob_emails[0].subject.contains("Mentioned by charlie"),
        "Bob should receive the mention email"
    );

    let alice_emails: Vec<_> = queued
        .iter()
        .filter(|e| e.email == "alice@test.com")
        .collect();
    assert_eq!(
        alice_emails.len(),
        1,
        "Reporter alice should receive the standard note notification"
    );
    assert!(
        alice_emails[0].subject.contains("Note added by charlie"),
        "Alice should receive the note notification"
    );
}

#[test]
fn test_chunked_batch_lookups_over_200_ids() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db.create_project("ChunkProj", 10, 10, "", true).unwrap();
    let uid = db
        .create_user("chunker", "Chunker", "chunk@x.com", "pw", 55, true, false)
        .unwrap();

    let mut bug_ids = Vec::with_capacity(250);
    for idx in 0..250 {
        let bid = db
            .create_bug(
                proj_id,
                uid,
                1,
                &format!("Chunk bug {}", idx),
                "desc",
                "",
                "",
                30,
                50,
                10,
                "",
                "",
                "",
                "",
                10,
            )
            .unwrap();
        bug_ids.push(bid);
    }

    let bugs_map = db.get_bugs_by_ids(&bug_ids).unwrap();
    assert_eq!(bugs_map.len(), 250);
    assert_eq!(bugs_map[&bug_ids[0]].summary, "Chunk bug 0");
    assert_eq!(bugs_map[&bug_ids[249]].summary, "Chunk bug 249");

    let target_bug = bug_ids[0];
    let mut note_ids = Vec::with_capacity(230);
    for idx in 0..230 {
        let nid = db
            .add_bugnote(target_bug, uid, &format!("Chunk note {}", idx), 10)
            .unwrap();
        note_ids.push(nid);
    }

    let notes_map = db.get_bugnotes_by_ids(&note_ids).unwrap();
    assert_eq!(notes_map.len(), 230);
    assert_eq!(notes_map[&note_ids[0]].note, "Chunk note 0");
    assert_eq!(notes_map[&note_ids[229]].note, "Chunk note 229");
}

#[test]
fn test_change_bug_status_note_mentions_and_exclude_dedup() {
    let db = Db::open_in_memory().unwrap();
    let proj_id = db
        .create_project("StatusMentionProj", 10, 10, "", true)
        .unwrap();
    let rep_id = db
        .create_user("rep_u", "Rep", "rep@test.com", "pw", 25, true, false)
        .unwrap();
    let dev_id = db
        .create_user("dev_u", "Dev", "dev@test.com", "pw", 55, true, false)
        .unwrap();
    let actor_id = db
        .create_user("actor_u", "Actor", "actor@test.com", "pw", 55, true, false)
        .unwrap();
    let _observer_id = db
        .create_user("obs_u", "Observer", "obs@test.com", "pw", 25, true, false)
        .unwrap();

    let bug_id = db
        .create_bug(
            proj_id,
            rep_id,
            1,
            "Status mention bug",
            "Desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    {
        let conn = db.conn();
        conn.execute(
            "UPDATE mantis_bug_table SET handler_id = ?1 WHERE id = ?2",
            rusqlite::params![dev_id, bug_id],
        )
        .unwrap();
        conn.execute("DELETE FROM mantis_email_table", []).unwrap();
    }

    // Resolving with a note mentioning @dev_u (handler) and @obs_u (non-subscriber):
    // - dev_u should receive ONLY 1 email ("Mentioned by actor_u"), NOT a duplicate status email.
    // - obs_u should receive 1 mention email.
    // - rep_u should receive 1 standard status-changed email.
    db.change_bug_status(
        bug_id,
        actor_id,
        80,
        20,
        Some("Fixed; please verify @dev_u and @obs_u"),
    )
    .unwrap();

    let queued = db.list_queued_emails().unwrap();
    assert_eq!(queued.len(), 3, "Expected exactly 3 emails: rep, dev, obs");

    let dev_emails: Vec<_> = queued
        .iter()
        .filter(|e| e.email == "dev@test.com")
        .collect();
    assert_eq!(dev_emails.len(), 1);
    assert!(dev_emails[0].subject.contains("Mentioned by actor_u"));
    assert!(dev_emails[0].body.contains("[Status changed to resolved]"));
    assert!(dev_emails[0].body.contains("Fixed; please verify @dev_u"));

    let obs_emails: Vec<_> = queued
        .iter()
        .filter(|e| e.email == "obs@test.com")
        .collect();
    assert_eq!(obs_emails.len(), 1);
    assert!(obs_emails[0].subject.contains("Mentioned by actor_u"));
    assert!(obs_emails[0].body.contains("[Status changed to resolved]"));

    let rep_emails: Vec<_> = queued
        .iter()
        .filter(|e| e.email == "rep@test.com")
        .collect();
    assert_eq!(rep_emails.len(), 1);
    assert!(rep_emails[0].subject.contains("Status changed to resolved"));

    // Also test change_bug_status_with_exclude (used when REST PATCH notifies description mentions + changes status)
    db.flush_email_queue().unwrap();
    db.change_bug_status_with_exclude(bug_id, actor_id, 90, 20, None, &["rep_u".to_string()])
        .unwrap();
    let queued2 = db.list_queued_emails().unwrap();
    assert_eq!(queued2.len(), 1);
    assert_eq!(queued2[0].email, "dev@test.com");
}

#[test]
fn test_timeline_events_rbac_isolation_and_note_counts() {
    let db = Db::open_in_memory().unwrap();
    let pub_proj = db.create_project("Public Proj", 10, 10, "", true).unwrap();
    let priv_proj = db.create_project("Private Proj", 10, 50, "", true).unwrap();

    let rep_id = db
        .create_user("rep_tl", "Reporter", "rep@tl.test", "pw", 25, true, false)
        .unwrap();
    let dev_id = db
        .create_user("dev_tl", "Developer", "dev@tl.test", "pw", 55, true, false)
        .unwrap();

    let rep_user = db.get_user_by_id(rep_id).unwrap().unwrap();
    let dev_user = db.get_user_by_id(dev_id).unwrap().unwrap();

    // Public bug in public project created by dev
    let pub_bug = db
        .create_bug(
            pub_proj,
            dev_id,
            1,
            "Public Issue",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Private bug in public project created by dev
    let priv_bug = db
        .create_bug(
            pub_proj,
            dev_id,
            1,
            "Private Issue",
            "secret",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            50,
        )
        .unwrap();

    // Public bug inside private project (rep_user is NOT a member of priv_proj!)
    let priv_proj_bug = db
        .create_bug(
            priv_proj,
            dev_id,
            1,
            "Hidden Project Issue",
            "secret",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Public note (with time_tracking=30) and private note (with time_tracking=45) on pub_bug
    let pub_note_id = db
        .add_bugnote_with_time(pub_bug, rep_id, "Public comment", 10, 30)
        .unwrap();
    let priv_note_id = db
        .add_bugnote_with_time(pub_bug, dev_id, "Private dev comment", 50, 45)
        .unwrap();

    // Update both notes and toggle view_state on public note (then toggle back to public)
    db.update_bugnote(pub_note_id, rep_id, "Public comment edited")
        .unwrap();
    db.update_bugnote(priv_note_id, dev_id, "Private dev comment edited")
        .unwrap();
    db.set_bugnote_view_state(pub_note_id, dev_id, 50).unwrap();
    db.set_bugnote_view_state(pub_note_id, dev_id, 10).unwrap();

    // Timeline for rep_user across All Projects (None):
    // - Sees pub_bug creation, public bugnote_added, and public bugnote_updated
    // - Must NOT see priv_bug, priv_proj_bug, private bugnote_added, private bugnote_updated, bugnote_view_state, or private time_tracking
    let rep_events = db.list_timeline_events(&rep_user, None, 50).unwrap();
    assert!(rep_events
        .iter()
        .any(|e| e.bug_id == pub_bug && e.type_ == 1 && e.field_name.is_empty()));
    assert!(rep_events
        .iter()
        .any(|e| e.bug_id == pub_bug && e.field_name == "bugnote_added"));
    assert_eq!(
        rep_events
            .iter()
            .filter(|e| e.bug_id == pub_bug && e.field_name == "bugnote_added")
            .count(),
        1,
        "Reporter must only see 1 public bugnote_added event"
    );
    assert_eq!(
        rep_events
            .iter()
            .filter(|e| e.bug_id == pub_bug && e.field_name == "bugnote_updated")
            .count(),
        1,
        "Reporter must only see 1 public bugnote_updated event"
    );
    assert!(
        !rep_events
            .iter()
            .any(|e| e.field_name == "bugnote_view_state"),
        "Reporter must not see bugnote_view_state events"
    );
    assert!(
        rep_events
            .iter()
            .any(|e| e.bug_id == pub_bug && e.field_name == "time_tracking" && e.new_value == "30"),
        "Reporter must see time_tracking event from public note"
    );
    assert!(
        !rep_events
            .iter()
            .any(|e| e.field_name == "time_tracking" && e.new_value == "45"),
        "Reporter must not see time_tracking events from private notes"
    );
    assert!(
        !rep_events.iter().any(|e| e.bug_id == priv_bug),
        "Reporter must not see private issue events"
    );
    assert!(
        !rep_events.iter().any(|e| e.bug_id == priv_proj_bug),
        "Reporter must not see private project issue events"
    );

    // Scoped to priv_proj: rep_user gets empty timeline
    assert!(db
        .list_timeline_events(&rep_user, Some(priv_proj), 50)
        .unwrap()
        .is_empty());

    // Timeline for dev_user (global access 55, no membership in priv_proj -> still cannot see priv_proj, CAN see priv_bug & private note in pub_proj)
    let dev_events = db
        .list_timeline_events(&dev_user, Some(pub_proj), 50)
        .unwrap();
    assert!(dev_events.iter().any(|e| e.bug_id == priv_bug));
    assert_eq!(
        dev_events
            .iter()
            .filter(|e| e.bug_id == pub_bug && e.field_name == "bugnote_added")
            .count(),
        2,
        "Developer must see both public and private bugnote_added events"
    );
    assert_eq!(
        dev_events
            .iter()
            .filter(|e| e.bug_id == pub_bug && e.field_name == "bugnote_updated")
            .count(),
        2,
        "Developer must see both public and private bugnote_updated events"
    );
    assert_eq!(
        dev_events
            .iter()
            .filter(|e| e.bug_id == pub_bug && e.field_name == "bugnote_view_state")
            .count(),
        2,
        "Developer must see bugnote_view_state events"
    );
    assert!(
        dev_events
            .iter()
            .any(|e| e.bug_id == pub_bug && e.field_name == "time_tracking" && e.new_value == "45"),
        "Developer must see time_tracking event on private note"
    );

    // Subproject scoping: parent pub_proj includes visible child subproject events, excludes non-member private subproject
    let sub_pub = db.create_project("Sub Public", 10, 10, "", true).unwrap();
    let sub_priv = db.create_project("Sub Private", 10, 50, "", true).unwrap();
    db.add_subproject(pub_proj, sub_pub, true).unwrap();
    db.add_subproject(pub_proj, sub_priv, true).unwrap();

    let sub_pub_bug = db
        .create_bug(
            sub_pub,
            dev_id,
            1,
            "Subproject Public Issue",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let sub_priv_bug = db
        .create_bug(
            sub_priv,
            dev_id,
            1,
            "Subproject Private Issue",
            "desc",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    let scoped_rep_events = db
        .list_timeline_events(&rep_user, Some(pub_proj), 50)
        .unwrap();
    assert!(scoped_rep_events.iter().any(|e| e.bug_id == sub_pub_bug));
    assert!(!scoped_rep_events.iter().any(|e| e.bug_id == sub_priv_bug));

    // Verify batch note counts respect private note visibility
    let mut rep_levels = HashMap::new();
    rep_levels.insert(pub_bug, 25);
    let (rep_notes, _) = db
        .get_bug_note_and_file_counts(&[pub_bug], &rep_levels)
        .unwrap();
    assert_eq!(rep_notes.get(&pub_bug).copied(), Some(1));

    let mut dev_levels = HashMap::new();
    dev_levels.insert(pub_bug, 55);
    let (dev_notes, _) = db
        .get_bug_note_and_file_counts(&[pub_bug], &dev_levels)
        .unwrap();
    assert_eq!(dev_notes.get(&pub_bug).copied(), Some(2));
}

#[test]
fn test_project_access_queries_match_effective_access_on_disabled_and_private_projects() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let rep_id = db
        .create_user("rep_sql", "Rep", "rep_sql@test.com", "pw", 25, true, false)
        .unwrap();
    let dev_id = db
        .create_user("dev_sql", "Dev", "dev_sql@test.com", "pw", 55, true, false)
        .unwrap();

    // Disabled project: admin still gets 90, project-override dev gets 0
    let dis_proj = db.create_project("DisabledProj", 10, 10, "", true).unwrap();
    db.update_project(dis_proj, "DisabledProj", 10, false, 10, "", true)
        .unwrap();
    db.set_project_user(dis_proj, dev_id, 55).unwrap();
    let dis_assignable = db.list_assignable_users_for_project(dis_proj, 55).unwrap();
    assert!(dis_assignable.iter().any(|u| u.id == admin.id));
    assert!(!dis_assignable.iter().any(|u| u.id == dev_id));
    let dis_map = db.list_project_user_access_map(dis_proj).unwrap();
    assert_eq!(dis_map.get(&admin.id).copied(), Some(90));
    assert_eq!(dis_map.get(&dev_id).copied(), Some(0));

    // Private project: non-member dev gets 0, explicitly added rep gets 25
    let priv_proj = db.create_project("PrivateProj", 10, 50, "", true).unwrap();
    db.set_project_user(priv_proj, rep_id, 25).unwrap();
    let priv_assignable = db.list_assignable_users_for_project(priv_proj, 55).unwrap();
    assert!(priv_assignable.iter().any(|u| u.id == admin.id));
    assert!(!priv_assignable.iter().any(|u| u.id == dev_id));
    assert!(!priv_assignable.iter().any(|u| u.id == rep_id));
    let priv_map = db.list_project_user_access_map(priv_proj).unwrap();
    assert_eq!(priv_map.get(&admin.id).copied(), Some(90));
    assert_eq!(priv_map.get(&dev_id).copied(), Some(0));
    assert_eq!(priv_map.get(&rep_id).copied(), Some(25));
}

#[test]
fn test_ui_m4_db_helpers_prefix_ancestors_and_overview_stats() {
    let db = Db::open_in_memory().unwrap();
    db.create_user("alice", "Alice", "a@x.com", "pw", 25, true, false)
        .unwrap();
    db.create_user("bob", "Bob", "b@x.com", "pw", 25, true, false)
        .unwrap();
    db.create_user("99bot", "Bot", "99@x.com", "pw", 25, true, false)
        .unwrap();

    let a_users = db.list_users_filtered(None, false, Some("A")).unwrap();
    assert!(a_users.iter().any(|u| u.username == "alice"));
    assert!(!a_users.iter().any(|u| u.username == "bob"));

    let num_users = db.list_users_filtered(None, false, Some("0-9")).unwrap();
    assert_eq!(num_users.len(), 1);
    assert_eq!(num_users[0].username, "99bot");

    let gp = db.create_project("GP", 10, 10, "", true).unwrap();
    let p = db.create_project("P", 10, 10, "", true).unwrap();
    let c = db.create_project("C", 10, 10, "", true).unwrap();
    db.add_subproject(gp, p, true).unwrap();
    db.add_subproject(p, c, true).unwrap();

    let ancestors = db.list_project_ancestor_ids(c).unwrap();
    assert!(ancestors.contains(&gp));
    assert!(ancestors.contains(&p));
    assert!(!ancestors.contains(&c));

    let stats = db.get_site_overview_stats().unwrap();
    assert_eq!(stats.projects_count, 3);
    assert!(stats.users_count >= 4);
}

#[test]
fn test_phase3_m3_my_view_config_commented_summary_and_billing_queries() {
    let db = Db::open_in_memory().unwrap();
    let admin = db.get_user_by_username("administrator").unwrap().unwrap();
    let dev_id = db
        .create_user("m3_dev", "M3 Dev", "m3d@x.com", "pw", 55, true, false)
        .unwrap();
    let dev = db.get_user_by_id(dev_id).unwrap().unwrap();

    // 1. My View config get / set / delete round-trip
    let def = db.get_my_view_config(dev_id);
    assert!(def.show_timeline);
    assert_eq!(def.enabled_boxes().len(), 6);

    let mut custom = def.clone();
    custom.show_timeline = false;
    for b in &mut custom.boxes {
        if b.key == "my_comments" {
            b.enabled = true;
            b.order = 1;
        } else {
            b.order += 1;
        }
    }
    db.set_my_view_config(dev_id, &custom).unwrap();
    let reloaded = db.get_my_view_config(dev_id);
    assert!(!reloaded.show_timeline);
    assert_eq!(
        reloaded.enabled_boxes()[0],
        crate::models::MyViewBox::MyComments
    );

    db.delete_config(crate::models::MY_VIEW_CONFIG_ID, 0, dev_id)
        .unwrap();
    assert!(db.get_my_view_config(dev_id).show_timeline);

    // 2. Projects & subprojects with private subproject RBAC isolation
    let parent_p = db.create_project("ParentBill", 10, 10, "", true).unwrap();
    let pub_child = db.create_project("PubChildBill", 10, 10, "", true).unwrap();
    let priv_child = db
        .create_project("PrivChildBill", 10, 50, "", true)
        .unwrap();
    db.add_subproject(parent_p, pub_child, true).unwrap();
    db.add_subproject(parent_p, priv_child, true).unwrap();

    let b_parent = db
        .create_bug(
            parent_p,
            1,
            1,
            "Parent Bug",
            "D",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let b_pub_child = db
        .create_bug(
            pub_child,
            1,
            1,
            "Pub Child Bug",
            "D",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    let b_priv_child = db
        .create_bug(
            priv_child,
            1,
            1,
            "Priv Child Bug",
            "D",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();

    // Commented bugs + history stats
    db.add_bugnote_with_time(b_parent, dev_id, "Dev note parent", 10, 60)
        .unwrap();
    db.add_bugnote_with_time(b_pub_child, dev_id, "Dev note pub child", 10, 120)
        .unwrap();
    db.add_bugnote_with_time(b_priv_child, admin.id, "Admin secret child", 10, 180)
        .unwrap();
    db.change_bug_status(b_parent, admin.id, 80, 20, Some("Fixed"))
        .unwrap();

    let commented = db.list_bugs_commented_by_user(&dev, None).unwrap();
    let commented_ids: Vec<i64> = commented.into_iter().map(|b| b.id).collect();
    assert!(commented_ids.contains(&b_parent));
    assert!(commented_ids.contains(&b_pub_child));
    assert!(!commented_ids.contains(&b_priv_child));

    let (hist_counts, resolved_at) = db
        .get_bug_history_summary_stats(&[b_parent, b_pub_child], &HashMap::new())
        .unwrap();
    assert!(hist_counts.get(&b_parent).copied().unwrap_or(0) >= 1);
    assert!(resolved_at.get(&b_parent).copied().unwrap_or(0) > 0);

    // Private note history is excluded for viewers < 55 and visible for >= 55
    let hist_priv_bug = db
        .create_bug(
            pub_child,
            admin.id,
            1,
            "Priv note activity bug",
            "",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.add_bugnote_with_time(hist_priv_bug, admin.id, "Hidden note", 50, 45)
        .unwrap();
    let mut low_map = HashMap::new();
    low_map.insert(hist_priv_bug, 25);
    let (low_hist, _) = db
        .get_bug_history_summary_stats(&[hist_priv_bug], &low_map)
        .unwrap();
    let mut dev_map = HashMap::new();
    dev_map.insert(hist_priv_bug, 55);
    let (dev_hist, _) = db
        .get_bug_history_summary_stats(&[hist_priv_bug], &dev_map)
        .unwrap();
    assert!(dev_hist[&hist_priv_bug] > low_hist[&hist_priv_bug]);

    // 3. Billing filtered: dev with include_subprojects=true sees parent + pub_child, NOT priv_child
    let dev_rollup = db
        .list_time_tracking_filtered(&dev, parent_p, true, None, None)
        .unwrap();
    let dev_bug_ids: Vec<i64> = dev_rollup.iter().map(|e| e.bug_id).collect();
    assert!(dev_bug_ids.contains(&b_parent));
    assert!(dev_bug_ids.contains(&b_pub_child));
    assert!(!dev_bug_ids.contains(&b_priv_child));

    // Direct parent only (include_subprojects=false)
    let dev_direct = db
        .list_time_tracking_filtered(&dev, parent_p, false, None, None)
        .unwrap();
    assert_eq!(dev_direct.len(), 1);
    assert_eq!(dev_direct[0].bug_id, b_parent);

    // Future date window returns empty
    let empty_future = db
        .list_time_tracking_filtered(&admin, parent_p, true, Some(4_000_000_000), None)
        .unwrap();
    assert!(empty_future.is_empty());

    // Private note is not visible to lower-privilege user filtering commented_by another user,
    // while project-scoped list_bugs_commented_by_user respects project_id & subprojects.
    let priv_note_bug = db
        .create_bug(
            pub_child,
            admin.id,
            1,
            "Bug with admin private note",
            "",
            "",
            "",
            30,
            50,
            10,
            "",
            "",
            "",
            "",
            10,
        )
        .unwrap();
    db.add_bugnote_with_time(priv_note_bug, admin.id, "Secret admin note", 50, 0)
        .unwrap();
    let rep_id = db
        .create_user("sub4_rep", "rep@ex.com", "Rep", "pw", 25, true, false)
        .unwrap();
    let rep = db.get_user_by_id(rep_id).unwrap().unwrap();
    let rep_filtering_admin = db
        .filter_bugs(
            &BugFilter {
                commented_by: Some(admin.id),
                ..Default::default()
            },
            &rep,
        )
        .unwrap();
    assert!(!rep_filtering_admin.iter().any(|b| b.id == priv_note_bug));
    assert!(!rep_filtering_admin.iter().any(|b| b.id == b_priv_child));

    let dev_scoped = db
        .list_bugs_commented_by_user(&dev, Some(pub_child))
        .unwrap();
    assert_eq!(dev_scoped.len(), 1);
    assert_eq!(dev_scoped[0].id, b_pub_child);

    // Oversized legacy time_tracking value (> i32::MAX) does not error out list_time_tracking_filtered
    {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO mantis_bugnote_table (bug_id, reporter_id, bugnote_text_id, view_state, date_submitted, last_modified, note_type, note_attr, time_tracking)
             VALUES (?1, ?2, 0, 10, 1700000000, 1700000000, 0, '', 3000000000)",
            params![b_parent, dev_id],
        )
        .unwrap();
    }
    let big_entries = db
        .list_time_tracking_filtered(&dev, parent_p, false, None, None)
        .unwrap();
    assert!(big_entries.iter().any(|e| e.minutes == i32::MAX));
    let notes_with_big = db.list_bugnotes(b_parent, 55).unwrap();
    assert!(notes_with_big.iter().any(|n| n.time_tracking == i32::MAX));
}
