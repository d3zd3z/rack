//! ZFS operations

use chrono::{Datelike, Timelike, Local};
use regex::{self, Regex};
use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::os::unix::io::{AsRawFd, FromRawFd};

use Result;
use checked::CheckedExt;

#[derive(Debug)]
pub struct Zfs {
    /// The snapshot prefix.  Different prefixes can be used at different times, which will result
    /// in independent snapshots.
    pub prefix: String,
    /// The filesystems found on the system.
    pub filesystems: Vec<Filesystem>,
    /// A re to match snapshot names.
    snap_re: Regex,
}

#[derive(Debug)]
pub struct Filesystem {
    pub name: String,
    pub snaps: Vec<String>,
    pub mount: String,
}

impl Zfs {
    /// Construct a new Zfs retrieving all of the filesystems that are found on this system.
    pub fn new(prefix: &str) -> Result<Zfs> {
        let quoted = regex::escape(prefix);
        let pat = format!("^{}(\\d{{4}})-([-\\d]+)$", quoted);
        let re = Regex::new(&pat)?;

        // Ask ZFS what all of the Filesystems are that it knows about.  Just get the names and
        // mountpoints (which will include all snapshots).  Order of the volumes seems to mostly be
        // lexicographically, at least in some kind of tree order.  The snapshots come out in the
        // order they were created.
        let out = Command::new("zfs")
            .args(&["list", "-H", "-t", "all", "-o", "name,mountpoint"])
            .stderr(Stdio::inherit())
            .checked_output()?;
        let buf = out.stdout;

        let mut builder = SnapBuilder::new();

        for line in BufReader::new(&buf[..]).lines() {
            let line = line?;
            let fields: Vec<_> = line.splitn(2, '\t').collect();
            if fields.len() != 2 {
                return Err(format!("zfs line doesn't have two fields: {:?}", line).into());
            }
            // fields[0] is now the volume/snap name, and fields[1] is the mountpoint.
            let vols: Vec<_> = fields[0].splitn(2, '@').collect();
            match vols.len() {
                1 => builder.push_volume(vols[0], fields[1]),
                2 => builder.push_snap(vols[0], vols[1]),
                _ => panic!("Unexpected zfs output"),
            }
        }
        let result = builder.into_sets();

        Ok(Zfs {
            prefix: prefix.to_string(),
            filesystems: result,
            snap_re: re,
        })
    }

    /// Determine the next snapshot number to use, under a given prefix.  The prefix should be a
    /// filesystem name (possibly top level) without a trailing slash.  All filesystems at this
    /// point and under will be considered when looking for volumes.
    pub fn next_under(&self, under: &str) -> Result<usize> {
        let mut next = 0;

        for fs in self.filtered(under)? {
            for snap in &fs.snaps {
                if let Some(caps) = self.snap_re.captures(snap) {
                    let num = caps.get(1).unwrap().as_str().parse::<usize>().unwrap();
                    if num + 1 > next {
                        next = num + 1;
                    }
                }
            }
        }

        Ok(next)
    }

    /// Given a snapshot name, return the number of that snapshot, if it matches the pattern,
    /// otherwise None.
    fn snap_number(&self, text: &str) -> Option<usize> {
        self.snap_re.captures(text).map(|caps| {
            caps.get(1).unwrap().as_str().parse::<usize>().unwrap()
        })
    }

    /// Return the filtered subset of the filesystems under a given prefix.  Collected into a
    /// vector for type simplicity.
    fn filtered<'a>(&'a self, under: &str) -> Result<Vec<&'a Filesystem>> {
        let re = Regex::new(&format!("^{}(/.*)?$", regex::escape(under)))?;

        Ok(self.filesystems.iter().filter(|x| re.is_match(&x.name)).collect())
    }

    /// Generate a snapshot name of the given index, and the current time.
    pub fn snap_name(&self, index: usize) -> String {
        let now = Local::now();
        let name = format!("{}{:04}-{:04}{:02}{:02}{:02}{:02}",
                           self.prefix, index,
                           now.year(), now.month(), now.day(),
                           now.hour(), now.minute());
        name
    }

    /// Make a new snapshot of the given index on the given filesystem name.  The snapshot itself
    /// will be made recursively.
    pub fn take_snapshot(&self, fs: &str, index: usize) -> Result<()> {
        let name = format!("{}@{}", fs, self.snap_name(index));
        println!("Make snapshot: {}", name);
        Command::new("zfs")
            .args(&["snapshot", "-r", &name])
            .checked_run()?;
        Ok(())
    }

    /// Clone one volume tree to another.
    pub fn clone(&self, source: &str, dest: &str) -> Result<()> {
        // Get filtered views of the source and destination filesystems under the given trees.
        let source_fs = self.filtered(source)?;
        let dest_fs = self.filtered(dest)?;

        // Make a mapping between the suffixes of the names (including the empty string for one
        // that exactly matches `dest`.  This should be safe as long as `.filtered()` above
        // always returns ones with this string as a prefix.
        let dest_map: HashMap<&str, &Filesystem> = dest_fs
            .iter().map(|&d| (&d.name[dest.len()..], d)).collect();

        for src in &source_fs {
            match dest_map.get(&src.name[source.len()..]) {
                Some(d) => {
                    println!("Clone existing: {:?} to {:?}", src.name, d.name);
                    self.clone_one(src, d)?;
                }
                None => {
                    println!("Clone fresh: {:?} {:?}+{:?}",
                             src.name, dest, &src.name[source.len()..]);

                    // Construct the new volume.
                    let destfs = Filesystem {
                        name: format!("{}{}", dest, &src.name[source.len()..]),
                        snaps: vec![],
                        mount: "*INVALID*".into(),
                    };

                    self.make_volume(src, &destfs)?;
                    self.clone_one(src, &destfs)?;
                }
            }
        }

        Ok(())
    }

    /// Clone a single filesystem to an existing volume.  We assume there are no snapshots on the
    /// destination that aren't on the source (otherwise it isn't possible to do the clone).
    fn clone_one(&self, source: &Filesystem, dest: &Filesystem) -> Result<()> {
        if let Some(ssnap) = dest.snaps.last() {
            if !source.snaps.contains(ssnap) {
                return Err("Last dest snapshot not present in source".into());
            }
            let dsnap = if let Some(dsnap) = source.snaps.last() {
                dsnap
            } else {
                return Err("Source volume has no snapshots".into());
            };

            if dsnap == ssnap {
                println!("Destination is up to date");
                return Ok(())
            }

            println!("Clone from {}@{} to {}@{}", source.name, ssnap, dest.name, dsnap);

            let size = self.estimate_size(&source.name, Some(ssnap), dsnap)?;
            println!("Estimate: {}", humanize_size(size));

            self.do_clone(&source.name, &dest.name, Some(ssnap), dsnap, size)?;

            Ok(())
        } else {
            // When doing a full clone, clone from the first snapshot of the volume, and then do a
            // differential backup from that snapshot.
            let dsnap = if let Some(dsnap) = source.snaps.first() {
                dsnap
            } else {
                return Err("Source volume has no snapshots".into());
            };

            println!("Full clone from {}@{} to {}", source.name, dsnap, dest.name);

            let size = self.estimate_size(&source.name, None, dsnap)?;
            println!("Estimate: {}", humanize_size(size));
            self.do_clone(&source.name, &dest.name, None, dsnap, size)?;

            // Run the clone on the rest of the image.
            let ssnap = dsnap;
            let dsnap = source.snaps.last().expect("source has first but no last");

            let size = self.estimate_size(&source.name, Some(ssnap), dsnap)?;
            println!("Estimate: {}", humanize_size(size));
            self.do_clone(&source.name, &dest.name, Some(ssnap), dsnap, size)?;

            Ok(())
        }
    }

    /// Use zfs send to estimate the size of this incremental backup.  If the source snap is none,
    /// operate as a full clone.
    fn estimate_size(&self, source: &str, ssnap: Option<&str>, dsnap: &str) -> Result<usize> {
        let mut cmd = Command::new("zfs");
        cmd.arg("send");
        cmd.arg("-nP");
        if let Some(ssnap) = ssnap {
            cmd.arg("-I");
            cmd.arg(&format!("@{}", ssnap));
        }
        cmd.arg(&format!("{}@{}", source, dsnap));
        cmd.stderr(Stdio::inherit());
        let out = cmd.checked_output()?;

        let buf = out.stdout;
        for line in BufReader::new(&buf[..]).lines() {
            let line = line?;
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() < 2 {
                return Err(format!("Invalid line from zfs send size estimate: {:?}", line).into());
            }
            if fields[0] != "size" {
                continue;
            }

            return Ok(fields[1].parse().unwrap());
        }

        Ok(0)
    }

    /// Perform the actual clone.
    fn do_clone(&self, source: &str, dest: &str, ssnap: Option<&str>, dsnap: &str, size: usize) -> Result<()> {
        // Construct a pipeline from zfs -> pv -> zfs.  PV is used to monitor the progress.
        let mut cmd = Command::new("zfs");
        cmd.arg("send");
        if let Some(ssnap) = ssnap {
            cmd.arg("-I");
            cmd.arg(&format!("@{}", ssnap));
        }
        cmd.arg(&format!("{}@{}", source, dsnap));
        cmd.stdout(Stdio::piped());
        let mut sender = cmd.spawn()?;

        let send_out = sender.stdout.as_ref().expect("Child output").as_raw_fd();

        // The unsafe is because using raw descriptors could make them available after they are
        // closed.  These are being given to a spawn, which will be inherited by a fork, and is
        // safe.
        let mut pv = Command::new("pv")
            .args(&["-s", &size.to_string()])
            .stdin(unsafe {Stdio::from_raw_fd(send_out)})
            .stdout(Stdio::piped())
            .spawn()?;

        let pv_out = pv.stdout.as_ref().expect("PV output").as_raw_fd();

        let mut receiver = Command::new("zfs")
            .args(&["receive", "-vFu", dest])
            .stdin(unsafe {Stdio::from_raw_fd(pv_out)})
            .spawn()?;

        // pv -s <size>
        // zfs receive -vFu <dest>

        if !sender.wait()?.success() {
            return Err(format!("zfs send error").into());
        }
        if !pv.wait()?.success() {
            return Err(format!("pv error").into());
        }
        if !receiver.wait()?.success() {
            return Err(format!("zfs receive error").into());
        }

        Ok(())
    }

    /// Prune old snapshots.  This is a Hanoi-type pruning model, where we keep the most recent
    /// snapshot that has the same number of bits set in it.  In addition, we keep a certain number
    /// `PRUNE_KEEP` of the most recent snapshots.
    pub fn prune(&self, fs_name: &str, really: bool) -> Result<()> {
        let fs = if let Some(fs) = self.filesystems.iter().find(|fs| fs.name == fs_name) {
            fs
        } else {
            return Err(format!("Volume not found in zfs {:?}", fs_name).into());
        };

        // Get all of the snapshots, oldest first, that match this tag, and pair them up with
        // the decoded number.
        let mut snaps: Vec<_> = fs.snaps.iter().filter_map(|sn| {
            self.snap_number(sn).map(|num| (sn, num))
        }).collect();
        snaps.reverse();

        let mut pops = BTreeSet::<u32>::new();
        let mut to_prune = vec![];

        for item in snaps.iter().enumerate() {

            // Don't prune the most recent ones.
            let index = item.0;
            if index < PRUNE_KEEP {
                continue;
            }

            let name = (item.1).0;
            let num = (item.1).1;

            let bit_count = num.count_ones();
            if pops.contains(&bit_count) {
                let prune_name = format!("{}@{}", fs_name, name);

                to_prune.push(prune_name);

            }
            pops.insert(bit_count);
        }

        // Now do the actual pruning, starting with the oldest ones.
        to_prune.reverse();

        for prune_name in &to_prune {
            println!("{}prune: {}", if really { "" } else { "would " }, prune_name);
            if really {
                Command::new("zfs")
                    .arg("destroy")
                    .arg(&prune_name)
                    .checked_run()?;
            }
        }

        Ok(())
    }

    /// Construct a new volume at "dest".  Copies over certain attributes (acltype, xattr, atime,
    /// relatime) that are relevant to the snapshot being correct.
    fn make_volume(&self, src: &Filesystem, dest: &Filesystem) -> Result<()> {
        // Read the attributes from the source volume.
        let out = Command::new("zfs")
            .args(&["get", "-Hp", "all", &src.name])
            .stderr(Stdio::inherit())
            .checked_output()?;
        let buf = out.stdout;
        let mut props = vec![];
        for line in BufReader::new(&buf[..]).lines() {
            let line = line?;
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() != 4 {
                return Err(format!("zfs get line doesn't have 4 fields: {:?}", line).into());
            }
            // 0 - name
            // 1 - property
            // 2 - value
            // 3 - source

            // We care about "local" or "received" properties, which are ones that will be set to a
            // value not present.  But, don't include the 'mountpoint' property, so that the backup
            // won't have things randomly mounted.
            if fields[1] == "mountpoint" {
                continue;
            }
            if fields[3] == "local" || fields[3] == "received" {
                props.push("-o".into());
                props.push(format!("{}={}", fields[1], fields[2]));
            }
        }
        println!("   props: {:?}", props);

        Command::new("zfs")
            .arg("create")
            .args(&props)
            .arg(&dest.name)
            .checked_run()?;

        Ok(())
    }
}

/// The number of recent ones to keep.
const PRUNE_KEEP: usize = 10;

/// A `SnapBuilder` is used to build up the snapshot view of filesystems.
struct SnapBuilder {
    work: Vec<Filesystem>,
}

impl SnapBuilder {
    fn new() -> SnapBuilder {
        SnapBuilder {
            work: vec![],
        }
    }

    fn into_sets(self) -> Vec<Filesystem> {
        self.work
    }

    fn push_volume(&mut self, name: &str, mount: &str) {
        self.work.push(Filesystem {
            name: name.to_owned(),
            snaps: vec![],
            mount: mount.to_owned(),
        });
    }

    fn push_snap(&mut self, name: &str, snap: &str) {
        let pos = self.work.len();
        if pos == 0 {
            panic!("Got snapshot from zfs before volume");
        }
        let set = &mut self.work[pos - 1];
        if name != set.name {
            panic!("Got snapshot from zfs without same volume name");
        }
        set.snaps.push(snap.to_owned());
    }
}

/// Humanize sizes with base-2 SI-like prefixes.
fn humanize_size(size: usize) -> String {
    // This unit table covers at least 80 bits, so the later ones will never be used.
    static UNITS: &'static [&'static str] = &[
        "B  ", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB" ];

    let mut value = size as f64;
    let mut unit = 0;

    while value > 1024.0 {
        value /= 1024.0;
        unit += 1;
    }

    let precision = if value < 10.0 {
        3
    } else if value < 100.0 {
        2
    } else {
        2
    };

    format!("{:6.*}{}", precision, value, UNITS[unit])
}
