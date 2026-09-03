//! Sharing one insight store between machines.
//!
//! The store is a directory of markdown files, so a git repository is all it
//! takes for the insights written on a laptop to reach a desktop. `insights
//! sync` commits what this machine wrote, replays it on top of what the other
//! machines wrote, and pushes the result.
//!
//! Only the insights travel. What a machine records about its own use of them
//! lives outside the store entirely (see `server::models::usage`), which is
//! what keeps reading an insight from producing a change to sync.

mod git;
mod report;
mod run;

pub use report::SyncReport;
pub use run::{forget_remote, init, sync};
