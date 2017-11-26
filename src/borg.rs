//! Borg backups

use Result;
use sync::MountedDir;

use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use zfs::{Filesystem, find_mount};

pub fn run(fs: &Filesystem, borg_repo: &str, name: &str) -> Result<()> {
    let out = Command::new("borg")
        .args(&["list", "--short", borg_repo])
        .stderr(Stdio::inherit())
        .output()?;
    if !out.status.success() {
        return Err(format!("Unable to run borg: {:?}", out.status).into());
    }
    let buf = out.stdout;

    let mut present = HashSet::new();
    for line in BufReader::new(&buf[..]).lines() {
        let line = line?;
        present.insert(line);
    }

    println!("Borg: {} snapshots to backup",
             fs.snaps.iter().filter(|x| !present.contains(&x[..])).count());

    // Go through all of the snapshots, in order, and back up ones that are missing.
    for snap in &fs.snaps {
        let snapname = format!("{}{}", name, snap);
        if present.contains(&snapname) {
            continue;
        }

        fs.borg_backup(borg_repo, snap, name)?;
    }

    Ok(())
}

impl Filesystem {
    fn borg_backup(&self, borg_repo: &str, snap: &str, name: &str) -> Result<()> {
        let mount = find_mount(&self.name)?;
        let dest = format!("{}/.zfs/snapshot/{}", mount, snap);

        // Stat "." in this directory to request ZFS automount the snapshot.
        let meta = fs::metadata(format!("{}/.", dest))?;
        if !meta.is_dir() {
            return Err(format!("Snapshot is not a directory: {:?}", dest).into());
        }

        // Bind mount to have consistent path for borg.  This needs to be specific to the given
        // filesystem.
        let srcdir = match name {
            "gentoo-" => "/mnt/root",
            "home-" => "/mnt/home",
            name => return Err(format!("Unsupported borg backup name: {:?}", name).into()),
        };
        let _root = MountedDir::new(&dest, Path::new(&srcdir))?;

        let archive = format!("{}::{}{}", borg_repo, name, snap);

        // Run the backup itself.
        println!("Backing up {:?} to {:?}", dest, archive);

        let status = Command::new("borg")
            .args(&["create", "-p", "--exclude-caches", &archive, &srcdir])
            .stderr(Stdio::inherit())
            .status()?;
        if !status.success() {
            return Err(format!("Error running borg: {:?}", status).into());
        }

        Ok(())
    }
}
