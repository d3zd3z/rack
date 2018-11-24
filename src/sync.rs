//! Sync the root filesystem to a volume on ZFS.

use std::{fs, path::Path, process::Command};

use crate::lvm::Lvm;
use crate::Result;
use crate::HOME_BIND_DIR;
use crate::ROOT_BIND_DIR;

/// Sync the root filesystem to a volume on ZFS.
///
/// The root filesystem on my system lives on ext4, mostly because of the added complexity of
/// having ZFS on root.  This used to just bind mount, but now that root is on lvm, we can make a
/// proper snapshot.
pub fn sync_root(root_fs: &str) -> Result<()> {
    let mut lvols = Lvm::scan("ubuntu-vg", "gentooroot")?;
    let snap = lvols.new_name();
    lvols.create_snapshot(&snap)?;

    let _root = lvols.mount_snapshot(&snap, ROOT_BIND_DIR)?;

    let status = Command::new("rsync")
        // .arg("-n")
        .arg("-aiHAX")
        .arg("--delete")
        .arg(&format!("{}/.", ROOT_BIND_DIR))
        .arg(&format!("/{}/.", root_fs))
        .status()?;
    if !status.success() {
        return Err(format_err!("Error running rsync: {:?}", status));
    }
    Ok(())
}

/// Sync the home filesystem to a volume on ZFS.
///
/// The home filesystem also lives on ext4, with a lvm thinvol snapshot.
pub fn sync_home(home_fs: &str) -> Result<()> {
    let mut lvols = Lvm::scan("ubuntu-vg", "home")?;
    let snap = lvols.new_name();
    lvols.create_snapshot(&snap)?;

    let _home = lvols.mount_snapshot(&snap, HOME_BIND_DIR)?;

    let status = Command::new("rsync")
        .arg("-aiHAX")
        .arg("--delete")
        .arg(&format!("{}/.", HOME_BIND_DIR))
        .arg(&format!("/{}/.", home_fs))
        .status()?;
    if !status.success() {
        return Err(format_err!("Error running rsync: {:?}", status));
    }
    Ok(())
}

// Ensure the named directory is empty, but exists.
fn ensure_empty<P: AsRef<Path>>(name: P) -> Result<()> {
    let name = name.as_ref();

    if !name.is_dir() {
        return Err(format_err!("Root {:?} is not a directory", name));
    }

    if let Some(entry) = fs::read_dir(name)?.next() {
        return Err(format_err!(
            "Root {:?} is not empty (has {:?})",
            name,
            entry?
        ));
    }

    Ok(())
}

// Bind mount a directory, making sure to unmount it when this value goes out of scope.
pub struct MountedDir<'a>(&'a Path);

impl<'a> MountedDir<'a> {
    pub fn new<P1: AsRef<Path>>(from: P1, to: &'a Path) -> Result<MountedDir<'a>> {
        ensure_empty(to)?;
        let from = from.as_ref();
        let status = Command::new("mount")
            .arg("--bind")
            .arg(from)
            .arg(to)
            .status()?;
        if !status.success() {
            return Err(format_err!("Error running mount command: {:?}", status));
        }
        Ok(MountedDir(to))
    }
}

impl<'a> Drop for MountedDir<'a> {
    fn drop(&mut self) {
        let status = Command::new("umount")
            .arg(self.0)
            .status()
            .expect("Umount command");
        if !status.success() {
            panic!("Error running unmount command");
        }
    }
}
