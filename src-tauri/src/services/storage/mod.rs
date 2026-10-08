#[path = "database/db.rs"]
pub mod db;
#[path = "tables/drafts.rs"]
pub mod drafts;
#[path = "tables/files.rs"]
pub mod files;
#[path = "tables/messages.rs"]
pub mod messages;
#[path = "tables/rules.rs"]
pub mod rules;
#[path = "database/schema.rs"]
pub mod schema;
#[path = "tables/settings.rs"]
pub mod settings;
#[path = "tables/summaries.rs"]
pub mod summaries;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;

pub use db::Database;
pub use files::GroupFileRecord;
pub use rules::ContactRuleRecord;
