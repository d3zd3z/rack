//! The "rack" backup library.
//!
//! Rack is a set of utilities written by David Brown to help him back up his computer.  It may or
//! may not be useful to anyone else.

// For development.
#![allow(dead_code)]

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate chrono;
#[macro_use] extern crate failure;
#[macro_use] extern crate failure_derive;
extern crate regex;
extern crate rsure;

use failure::err_msg;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::process::ExitStatus;
use std::result;

mod checked;
mod borg;
mod lvm;
mod sync;
mod zfs;

use zfs::Zfs;

/// Local error type.
#[derive(Fail, Debug)]
enum RackError {
    #[fail(display = "error running command: {:?}: {}", status, command)]
    Command {
        command: String,
        status: ExitStatus,
    },
    #[fail(display = "not mounted: {:?}", fs)]
    NotMounted {
        fs: String,
    },
}

pub type Result<T> = result::Result<T, Error>;
pub type Error = failure::Error;

/// The path where root will be temporarily bind mounted.
static ROOT_BIND_DIR: &'static str = "/mnt/root";

/// The path where home will be temporarily mounted.
static HOME_BIND_DIR: &'static str = "/mnt/home";

pub use sync::{sync_home, sync_root};

/// Make a snapshot of some useful volumes.
pub fn snapshot(prefix: &str, filesystem: &str) -> Result<()> {
    let snap = Zfs::new(prefix)?;
    // println!("snap: {:?}", snap);
    let next = snap.next_under(filesystem)?;
    println!("next: {}: {}", next, snap.snap_name(next));
    snap.take_snapshot(filesystem, next)?;
    Ok(())
}

/// Clone one volume to another.
pub fn clone(source: &str, dest: &str, perform: bool, excludes: &[&str]) -> Result<()> {
    println!("Cloning {} to {}", source, dest);
    let snap = Zfs::new("caz")?;
    snap.clone(source, dest, perform, excludes)?;

    Ok(())
}

/// Prune backups.  Expire old backups according to pruning rules.  If `really` is true, actually
/// do the pruning, instead of just printing the names.
pub fn prune(prefix: &str, filesystem: &str, really: bool) -> Result<()> {
    let snap = Zfs::new(prefix)?;
    snap.prune(filesystem, really)?;
    Ok(())
}

/// Update sure data for existing snapshots.
pub fn sure(prefix: &str, filesystem: &str, surefile: &str) -> Result<()> {
    let snap = Zfs::new(prefix)?;

    // A regex to filter snapshots matching the desired prefix.
    let quoted = regex::escape(prefix);
    let pat = format!(r"^{}\d{{4}}-[-\d]+$", quoted);
    let re = Regex::new(&pat)?;

    // Find the filesystem that matches
    let fs = if let Some(fs) = snap.filesystems.iter().find(|&fs| fs.name == filesystem) {
        fs
    } else {
        return Err(err_msg("No snapshots match"));
    };

    let snaps: Vec<_> = fs.snaps.iter().filter(|x| re.is_match(x)).collect();

    // println!("Snaps: {:?}", snaps);
    // println!("Mountpoint: {:?}", fs.mount);

    let store = rsure::parse_store(surefile)?;
    let versions = store.get_versions()?;

    let versions: Vec<_> = versions.iter().filter(|x| re.is_match(&x.name)).collect();
    let verset: HashSet<&String> = versions.iter().map(|x| &x.name).collect();

    // println!("Sure versions: {:?}", versions.iter().map(|x| &x.name).collect::<Vec<_>>());

    // Go through the snapshots, in order, showing any that haven't been rsured.  If ones in the
    // middle are not present, we should really base off of those, but in the normal case, this
    // will always just add ones at the end.
    for vers in &snaps {
        if verset.contains(vers) {
            continue;
        }

        println!("Capture: {:?}", vers);
        // Although ZFS tells us where it thinks things should be mounted,
        // it isn't always right, instead find out where Linux view the
        // mounpoints.
        let mount = snap.find_mount(&fs.name)?;

        // Zfs snapshots seem to not mount until something inside is read.  It seems sufficient to
        // stat "." in the root (but no the root directory itself).
        let base = Path::new(&mount).join(".zfs").join("snapshot").join(vers);
        let dotfile = base.join(".");
        let _ = dotfile.metadata()?;
        println!("Stat {:?} for {:?}", dotfile, base);
        let mut tags = rsure::StoreTags::new();
        tags.insert("name".into(), vers.to_string());
        rsure::update(base, &*store, true, &tags)?;
    }

    Ok(())
}

pub fn run_borg(filesystem: &str, borg_repo: &str, name: &str) -> Result<()> {
    let snap = Zfs::new(filesystem)?;

    let fs = if let Some(fs) = snap.filesystems.iter().find(|&fs| fs.name == filesystem) {
        fs
    } else {
        return Err(err_msg("No snapshots match"));
    };

    // Just get the snapshots matching this single prefix.
    borg::run(fs, borg_repo, name).unwrap();

    Ok(())
}
