//! The "rack" backup library.
//!
//! Rack is a set of utilities written by David Brown to help him back up his computer.  It may or
//! may not be useful to anyone else.

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

use std::error;
use std::result;

mod sync;

/// Our local result type, for now, just box the errors up.
pub type Result<T> = result::Result<T, Box<error::Error + Send + Sync>>;

/// The path where root will be temporarily bind mounted.
static ROOT_BIND_DIR: &'static str = "/mnt/root";

/// The volume where root will be mirrored.
static ROOT_DEST: &'static str = "/lint/ext4root";

pub use sync::sync_root;
