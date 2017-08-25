//! Sync the root filesystem to a volume on ZFS.

use std::fs;
use std::path::Path;
use std::process::Command;

use Result;
use ROOT_BIND_DIR;
use ROOT_DEST;

/// Sync the root filesystem to a volume on ZFS.
///
/// The root filesystem on my system lives on ext4, mostly because of the added complexity of
/// having ZFS on root.  To back that up, we bind mount it to a tmp area, rsync that, and then
/// remove the bind mount.
pub fn sync_root() -> Result<()> {
    let _root = MountedDir::new("/", Path::new(ROOT_BIND_DIR))?;

    let status = Command::new("rsync")
        // .arg("-n")
        .arg("-aiHAX")
        .arg("--delete")
        .arg(&format!("{}/.", ROOT_BIND_DIR))
        .arg(&format!("{}/.", ROOT_DEST))
        .status()?;
    if !status.success() {
        return Err(format!("Error running rsync: {:?}", status).into());
    }
    Ok(())
}

// Ensure the named directory is empty, but exists.
fn ensure_empty<P: AsRef<Path>>(name: P) -> Result<()> {
    let name = name.as_ref();

    if !name.is_dir() {
        return Err(format!("Root {:?} is not a directory", name).into());
    }

    if let Some(entry) = fs::read_dir(name)?.next() {
        return Err(format!("Root {:?} is not empty (has {:?})", name, entry?).into());
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
            return Err(format!("Error running mount command: {:?}", status).into());
        }
        Ok(MountedDir(to))
    }
}

impl<'a> Drop for MountedDir<'a> {
    fn drop(&mut self) {
        let status = Command::new("umount")
            .arg(self.0)
            .status().expect("Umount command");
        if !status.success() {
            panic!("Error running unmount command");
        }
    }
}
