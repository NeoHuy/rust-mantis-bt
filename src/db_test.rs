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

    let bugs = db.list_bugs(proj_id, 10, 0, 25).unwrap();
    assert_eq!(bugs.len(), 1);

    // Private bug filtering at SQL level
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
    // Viewer (10) only sees public bug
    assert_eq!(db.list_bugs(proj_id, 10, 0, 25).unwrap().len(), 1);
    // Developer (55) sees both
    assert_eq!(db.list_bugs(proj_id, 55, 0, 25).unwrap().len(), 2);

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
    assert_eq!(db.list_bug_monitors(b2).unwrap().len(), 1);

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
    assert_eq!(link.created_by, admin.id);

    // Duplicate create fails
    assert!(db
        .create_jira_link(
            bug_id,
            "PROJ-102",
            "https://jira.example.com/browse/PROJ-102",
            "Task",
            admin.id
        )
        .is_err());

    // Delete link
    let deleted = db.delete_jira_link_by_bug_id(bug_id).unwrap();
    assert!(deleted);
    assert!(db.get_jira_link_by_bug_id(bug_id).unwrap().is_none());
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
