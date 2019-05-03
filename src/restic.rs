//! Backups using restic

use crate::{
    config::{Config, ResticConfig, ResticVolume},
    Result,
    sync::MountedDir,
    zfs::{find_mount, Filesystem, Zfs},
};
use failure::{err_msg, format_err};
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

        let snaps = self.get_snapshots()?;

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

    fn add_auth(&self, cmd: &mut Command) -> Result<()> {
        for au in &self.auth {
            let fields: Vec<_> = au.splitn(2, "=").collect();
            if fields.len() != 2 {
                return Err(format_err!("auth in config file is not KEY=value"));
            }
            cmd.env(fields[0], fields[1]);
        }

        Ok(())
    }

    /// Collect all of the snapshots contained within a particular restic
    /// backup.
    fn get_snapshots(&self) -> Result<Vec<Snapshot>> {
        let mut cmd = Command::new(RESTIC_BIN);
        cmd.args(&["-r", &self.repo, "snapshots", "--json"]);
        cmd.stderr(Stdio::inherit());
        self.add_auth(&mut cmd)?;
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(format_err!("Unable to run restic: {:?}", out.status));
        }
        let buf = out.stdout;

        Ok(serde_json::from_slice(&buf)?)
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
        let mut cmd = Command::new(RESTIC_BIN);
        cmd.args(&["-r", &rvol.repo,
                 "backup", "--exclude-caches",
                 "--tag", snap,
                 "--time", &fix_time(snap),
                 &rvol.bind]);
        rvol.add_auth(&mut cmd)?;
        let status = cmd.status()?;

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

impl Config {
    pub fn restic_prune(&self, really: bool) -> Result<()> {
        // Collect all of the restic snapshots.
        let rsnaps = self.restic.get_snaps()?;

        let zfs = Zfs::new("none")?;

        // Go through the snapshots themselves, pruning any that aren't
        // present in the restic snapshots.
        for vol in &self.snap.volumes {
            // Find the restic bind directory this was backed up under.
            let bind = self.restic.find_bind(&vol.zfs)?;
            println!("{:?}: {:?}", bind, vol);

            // Find the filesystem in ZFS.
            let fs = if let Some(fs) = zfs.filesystems.iter().find(|&fs| fs.name == vol.zfs) {
                fs
            } else {
                return Err(err_msg("No snapshots match"));
            };

            // Go through each snapshot in zfs, and if not present in a
            // restic backup, prune it.
            for snap in &fs.snaps {
                if !rsnaps.contains(&ResticSnap {
                    path: bind.clone(),
                    tag: snap.to_owned()
                }) {
                    zfs.prune(&vol.zfs, snap, really)?;
                } else {
                    println!(" keep {:?}@{:?}", vol.zfs, snap);
                }
            }
        }

        Ok(())
    }
}

impl ResticConfig {
    fn get_snaps(&self) -> Result<HashSet<ResticSnap>> {
        let mut rsnaps = HashSet::new();

        for v in &self.volumes {
            let snaps = v.get_snapshots()?;

            // Collect all of the involved snapshots.  Collect them by path
            // and tag.
            for snap in &snaps {
                for path in &snap.paths {
                    for tag in &snap.tags {
                        for tag in tag {
                            rsnaps.insert(ResticSnap {
                                path: path.to_owned(),
                                tag: tag.to_owned(),
                            });
                        }
                    }
                }
            }
        }
        Ok(rsnaps)
    }

    // Find the bind point this zfs volume is backed up to.  Will raise an
    // error if it is either not backed up, or if it has been backed up
    // under multiple bindings.
    fn find_bind(&self, zfs: &str) -> Result<String> {
        let binds: Vec<_> = self.volumes.iter().filter(|v| v.zfs == zfs).collect();
        match binds.len() {
            1 => Ok(binds[0].bind.clone()),
            0 => Err(format_err!("No restic backups found for zfs {:?}", zfs)),
            _ => {
                // Multiple backups are fine, as long as they all use the
                // same binding.
                let result = binds[0].bind.clone();
                for b in &binds[1..] {
                    if result != b.bind {
                        return Err(format_err!("Restic backups for zfs {:?} have different bind", zfs));
                    }
                }
                Ok(result)
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash)]
struct ResticSnap {
    path: String,
    tag: String,
}
