#[path = "accounts/accounts.rs"]
pub mod accounts;
#[path = "accounts/bootstrap.rs"]
pub mod bootstrap;
pub mod contacts;
#[path = "accounts/instances.rs"]
pub mod instances;
pub mod machine;
#[path = "accounts/migration.rs"]
pub mod migration;

#[cfg(test)]
mod tests;

pub use accounts::*;
pub use contacts::*;
pub use instances::*;
