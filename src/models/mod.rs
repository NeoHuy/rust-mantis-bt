pub mod api;
pub mod bug;
pub mod custom_field;
pub mod filter;
pub mod jira;
pub mod planning;
pub mod project;
pub mod tag;
pub mod user;

pub use api::*;
pub use bug::*;
pub use custom_field::*;
pub use filter::*;
pub use jira::*;
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
}
