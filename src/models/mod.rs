pub mod api;
pub mod bug;
pub mod custom_field;
pub mod filter;
pub mod planning;
pub mod project;
pub mod tag;
pub mod user;

pub use api::*;
pub use bug::*;
pub use custom_field::*;
pub use filter::*;
pub use planning::*;
pub use project::*;
pub use tag::*;
pub use user::*;

#[cfg(test)]
pub mod tests {
    use super::*;

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
}
