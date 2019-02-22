//! Backups using restic

use crate::{
    config::ResticVolume,
    Result,
    sync::MountedDir,
    zfs::{find_mount, Filesystem},
};
use failure::format_err;
use regex::Regex;
use serde_derive::{Deserialize};
use std::{
    collections::HashSet,
    fs,
    path::Path,
    process::{Command, Stdio},
};

// Mirrors the json that comes from the `restic snapshot --json` command.
#[derive(Debug, Deserialize)]
struct Snapshot {
    tree: String,
    short_id: String,
    paths: Vec<String>,
    time: String,
    parent: Option<String>,
    id: String,
    hostname: String,
    username: String,
    tags: Option<Vec<String>>,
}

pub struct Limiter(pub Option<usize>);

impl Limiter {
    fn exhausted(&mut self) -> bool {
        match self.0 {
            None => false,
            Some(0) => true,
            Some(ref mut n) => {
                *n -= 1;
                false
            }
        }
    }
}

static RESTIC_BIN: &'static str = "/home/davidb/bin/restic";

impl ResticVolume {
    pub fn run(&self, fs: &Filesystem, limit: &mut Limiter, pretend: bool) -> Result<()> {
        println!("Restic: {:?} {}", self, pretend);

        let out = Command::new(RESTIC_BIN)
            .args(&["-r", &self.repo, "-p", &self.passwordfile, "snapshots", "--json"])
            .stderr(Stdio::inherit())
            .output()?;
        if !out.status.success() {
            return Err(format_err!("Unable to run restic: {:?}", out.status));
        }
        let buf = out.stdout;

        let snaps: Vec<Snapshot> = serde_json::from_slice(&buf)?;

        // For every snapshot, where the 'paths' contains the bind for the
        // filesystem we are concerned with, add the tags to the list of
        // tags we have captured.
        let mut seen_tags = HashSet::new();
        for s in &snaps {
            if s.paths.iter().any(|p| p == &self.bind) {
                if let Some(ref tags) = s.tags {
                    for t in tags {
                        seen_tags.insert(t.to_owned());
                    }
                }
            }
        }
        // println!("restic: {:?}", seen_tags);
        // println!("zfs: {:?}", fs);

        // We'll need to back up every zfs snapshot that isn't present in
        // restic.
        for zsnap in &fs.snaps {
            if seen_tags.contains(zsnap) {
                continue;
            }

            if limit.exhausted() {
                break;
            }

            println!("Restic dump {:?} snapshot {:?}", self.zfs, zsnap);

            if pretend {
                continue;
            }

            fs.restic_backup(self, zsnap)?;
        }

        Ok(())
    }
}

impl Filesystem {
    fn restic_backup(&self, rvol: &ResticVolume, snap: &str) -> Result<()> {
        let mount = find_mount(&self.name)?;
        let dest = format!("{}/.zfs/snapshot/{}", mount, snap);

        // Stat "." in this directory to request ZFS automount the
        // snapshot.
        let meta = fs::metadata(format!("{}/.", dest))?;
        if !meta.is_dir() {
            return Err(format_err!("Snapshot is not a directory: {:?}", dest));
        }

        // Bind mount to have a consistent path for restic.  This needs to
        // be specific to the given filesystem.
        println!("Bind mount: {:?} from {:?}", dest, &rvol.bind);
        let _root = MountedDir::new(&dest, Path::new(&rvol.bind))?;

        // Run the actual restic command.
        let status = Command::new(RESTIC_BIN)
            .args(&["-r", &rvol.repo, "-p", &rvol.passwordfile,
                  "backup", "--exclude-caches",
                  "--tag", snap,
                  "--time", &fix_time(snap),
                  &rvol.bind])
            .status()?;

        if !status.success() {
            return Err(format_err!("Unable to run restic: {:?}", status));
        }

        Ok(())
    }
}

fn fix_time(snap: &str) -> String {
    let re = Regex::new(r".*(\d{4})(\d\d)(\d\d)(\d\d)(\d\d)$").unwrap();

    match re.captures(snap) {
        Some(cap) => {
            let year = cap.get(1).unwrap().as_str();
            let month = cap.get(2).unwrap().as_str();
            let day = cap.get(3).unwrap().as_str();
            let hour = cap.get(4).unwrap().as_str();
            let min = cap.get(5).unwrap().as_str();

            format!("{}-{}-{} {}:{}:00", year, month, day, hour, min)
        }
        None => "now".to_string()
    }
}
