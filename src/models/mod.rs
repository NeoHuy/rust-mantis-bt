pub mod api;
pub mod bug;
pub mod custom_field;
pub mod filter;
pub mod jira;
pub mod mentions;
pub mod planning;
pub mod project;
pub mod tag;
pub mod user;

pub use api::*;
pub use bug::*;
pub use custom_field::*;
pub use filter::*;
pub use jira::*;
pub use mentions::*;
pub use planning::*;
pub use project::*;
pub use tag::*;
pub use user::*;

// Standard MantisBT Access Levels
pub const ACCESS_VIEWER: i32 = 10;
pub const ACCESS_REPORTER: i32 = 25;
pub const ACCESS_UPDATER: i32 = 40;
pub const ACCESS_DEVELOPER: i32 = 55;
pub const ACCESS_MANAGER: i32 = 70;
pub const ACCESS_ADMINISTRATOR: i32 = 90;

// Standard MantisBT View States
pub const VIEW_STATE_PUBLIC: i32 = 10;
pub const VIEW_STATE_PRIVATE: i32 = 50;

// Standard MantisBT Issue Statuses
pub const STATUS_NEW: i32 = 10;
pub const STATUS_FEEDBACK: i32 = 20;
pub const STATUS_ACKNOWLEDGED: i32 = 30;
pub const STATUS_CONFIRMED: i32 = 40;
pub const STATUS_ASSIGNED: i32 = 50;
pub const STATUS_RESOLVED: i32 = 80;
pub const STATUS_CLOSED: i32 = 90;

// Well-known Plugin Identifiers
pub const PLUGIN_CORE_FORMATTING: &str = "MantisCoreFormatting";
pub const TOGGLEABLE_CORE_PLUGINS: &[&str] = &[PLUGIN_CORE_FORMATTING];

#[cfg(test)]
#[path = "models_test.rs"]
mod tests;
