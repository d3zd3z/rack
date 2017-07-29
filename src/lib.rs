//! The "rack" backup library.
//!
//! Rack is a set of utilities written by David Brown to help him back up his computer.  It may or
//! may not be useful to anyone else.

// For development.
#![allow(dead_code)]

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate chrono;
extern crate regex;

use std::error;
use std::result;

mod sync;
mod zfs;

use zfs::Zfs;

/// Our local result type, for now, just box the errors up.
pub type Result<T> = result::Result<T, Box<error::Error + Send + Sync>>;

/// The path where root will be temporarily bind mounted.
static ROOT_BIND_DIR: &'static str = "/mnt/root";

/// The volume where root will be mirrored.
static ROOT_DEST: &'static str = "/lint/ext4root";

pub use sync::sync_root;

/// Make a snapshot of some useful volumes.
pub fn snapshot() -> Result<()> {
    let snap = Zfs::new("caz")?;
    // println!("snap: {:?}", snap);
    let next = snap.next_under("lint/ext4root")?;
    println!("next: {}: {}", next, snap.snap_name(next));
    snap.take_snapshot("lint/ext4root", next)?;
    Ok(())
}
