//! The "rack" backup library.
//!
//! Rack is a set of utilities written by David Brown to help him back up his computer.  It may or
//! may not be useful to anyone else.

// For development.
#![allow(dead_code)]

#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate chrono;
#[macro_use] extern crate error_chain;
extern crate regex;
extern crate rsure;

use regex::Regex;
use std::collections::HashSet;
use std::io;
use std::path::Path;

mod sync;
mod zfs;

use zfs::Zfs;

/// Local error type.
error_chain! {
    types {
        Error, ErrorKind, ChainErr, Result;
    }

    links {
        Rsure(rsure::Error, rsure::ErrorKind);
    }

    foreign_links {
        Io(io::Error);
        Regex(regex::Error);
    }
}

/// The path where root will be temporarily bind mounted.
static ROOT_BIND_DIR: &'static str = "/mnt/root";

/// The volume where root will be mirrored.
static ROOT_DEST: &'static str = "/lint/ext4root";

pub use sync::sync_root;

/// Make a snapshot of some useful volumes.
pub fn snapshot(prefix: &str) -> Result<()> {
    let snap = Zfs::new(prefix)?;
    // println!("snap: {:?}", snap);
    let next = snap.next_under("lint/ext4root")?;
    println!("next: {}: {}", next, snap.snap_name(next));
    snap.take_snapshot("lint/ext4root", next)?;
    Ok(())
}

/// Clone one volume to another.
pub fn clone(source: &str, dest: &str) -> Result<()> {
    println!("Cloning {} to {}", source, dest);
    let snap = Zfs::new("caz")?;
    snap.clone(source, dest)?;

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
        return Err("No snapshots match".into());
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

        // Zfs snapshots seem to not mount until something inside is read.  It seems sufficient to
        // stat "." in the root (but no the root directory itself).
        let base = Path::new(&fs.mount).join(".zfs").join("snapshot").join(vers);
        let dotfile = base.join(".");
        let _ = dotfile.metadata()?;
        println!("Stat {:?} for {:?}", dotfile, base);
        let mut tags = rsure::StoreTags::new();
        tags.insert("name".into(), vers.to_string());
        rsure::update(base, &*store, true, &tags)?;
    }

    Ok(())
}
