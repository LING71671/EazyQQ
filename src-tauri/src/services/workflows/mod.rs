#[path = "files/archive.rs"]
pub mod archive;
#[path = "monitoring/chain.rs"]
pub mod chain;
#[path = "files/document.rs"]
pub mod document;
#[path = "files/file_summary.rs"]
pub mod file_summary;
#[path = "files/group_files.rs"]
pub mod group_files;
pub mod replies;
#[path = "summaries/scheduler.rs"]
pub mod scheduler;
#[path = "summaries/scheduler_settings.rs"]
pub mod scheduler_settings;
#[path = "summaries/summarizer.rs"]
pub mod summarizer;
pub mod tasks;

#[cfg(test)]
#[path = "tests/tests.rs"]
mod tests;
