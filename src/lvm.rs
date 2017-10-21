//! Manage lvm snapshots.

use chrono::{Datelike, Local};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use Result;
use checked::CheckedExt;

#[derive(Debug)]
pub struct Lvm {
    vg: String,
    lv: String,
    snaps: Vec<String>,
}

impl Lvm {
    /// Scan the system for LVM partitions releated to the specified one.
    pub fn scan(vg: &str, lv: &str) -> Result<Lvm> {
        let mut cmd = Command::new("lvs");
        cmd.args(&["--nameprefixes", "--noheadings", "--all", "--units", "b", "--nosuffix"]);
        cmd.stderr(Stdio::inherit());
        let out = cmd.output()?;
        if !out.status.success() {
            return Err(format!("lvs returned error: {:?}", out.status).into());
        }
        let buf = out.stdout;

        let mut main = None;
        let mut snaps = vec![];

        for line in BufReader::new(&buf[..]).lines() {
            let line = line?;
            // println!("line: {:?}", line);
            let fields = parse(&line);

            // We care about either the named vg (which should have no origin), or ones that
            // reference this as an origin.
            if fields.get("LVM2_VG_NAME").map(|x| x.as_str()) != Some(vg) {
                continue;
            }

            if fields.get("LVM2_LV_NAME").map(|x| x.as_str()) == Some(lv) &&
                fields.get("LVM2_ORIGIN").map(|x| x.as_str()) == Some("")
            {
                if main.is_some() {
                    panic!("Duplicate record for vg/lv");
                }
                main = Some(fields);
            } else if fields.get("LVM2_ORIGIN").map(|x| x.as_str()) == Some(lv) {
                snaps.push(fields);
            }
        }

        let main = main.expect("VG not present");

        Ok(Lvm {
            vg: main.get("LVM2_VG_NAME").expect("vg_name field").clone(),
            lv: main.get("LVM2_LV_NAME").expect("lv_name field").clone(),
            snaps: snaps.iter().map(|x| {
                x.get("LVM2_LV_NAME").expect("lv_name in snapshot").clone()
            }).collect()
        })
    }

    /// Come up with a snapshot name based on the date that isn't already taken.  Tries just
    /// appending -yyyy-mm-dd, and if that isn't unique appends letters (a-z, then aa-zz, etc).
    pub fn new_name(&self) -> String {
        let all: HashSet<&str> = self.snaps.iter().map(|x| x.as_str()).collect();

        let now = Local::now();
        let base = format!("{}-{:04}-{:02}-{:02}",
                           self.lv, now.year(), now.month(), now.day());

        // The simple case of a unique name.
        if !all.contains(base.as_str()) {
            return base;
        }

        for tail in SuffixGen::new() {
            let name = format!("{}{}", base, tail);

            if !all.contains(name.as_str()) {
                return name;
            }
        }
        unreachable!();
    }

    /// Create a new lvm snapshot of the given name.
    pub fn create_snapshot(&mut self, name: &str) -> Result<()> {
        let origin = format!("{}/{}", self.vg, self.lv);
        Command::new("lvcreate")
            .args(&["-s", "-n", name, &origin])
            .checked_run()?;

        // Add this snapshot to our list.
        self.snaps.push(name.to_string());
        Ok(())
    }

    /// Mount the given LV snapshot, returning an object that will unmount it when dropped.
    pub fn mount_snapshot(&self, name: &str, mountpoint: &str) -> Result<SnapMount> {
        SnapMount::mount(self, name.to_owned(), mountpoint.to_owned())
    }
}

/// A suffix generator.  Generates strings of the form "a" - "z", then "aa" - "zz".
struct SuffixGen {
    suffix: u32,
    digits: u32,
    cap: u32,
}

impl SuffixGen {
    fn new() -> SuffixGen {
        SuffixGen {
            suffix: 0,
            digits: 1,
            cap: 26,
        }
    }
}

impl Iterator for SuffixGen {
    type Item = String;
    fn next(&mut self) -> Option<String> {
        let mut result = String::new();

        let mut tmp = self.suffix;
        for _ in 0..self.digits {
            let ch = 'a' as u32 + tmp % 26;
            result.insert(0, ::std::char::from_u32(ch).unwrap());
            tmp /= 26;
        }

        self.suffix += 1;
        if self.suffix == self.cap {
            self.suffix = 0;
            self.digits += 1;
            self.cap *= 26;
        }

        Some(result)
    }
}

/// State to track a mounted snapshot.  Is able to unmount and clean up upon drop.
pub struct SnapMount {
    lvm_name: String,
    mountpoint: String,
    mounted: bool,
}

impl SnapMount {
    fn mount(lvm: &Lvm, name: String, mountpoint: String) -> Result<SnapMount> {
        let lvm_name = format!("{}/{}", lvm.vg, name);

        // Activate the lv
        Command::new("lvchange")
            .args(&["-ay", "-K", &lvm_name])
            .checked_run()?;

        // At this point, we have something to undo, so create our struct.
        let mut me = SnapMount {
            lvm_name: lvm_name,
            mountpoint: mountpoint,
            mounted: false,
        };

        let devname = format!("/dev/{}", me.lvm_name);
        // Run fsck.
        Command::new("fsck")
            .args(&["-p", &devname])
            .checked_run()?;

        // Mount the filesystem.
        Command::new("mount")
            .args(&["-r", &devname, &me.mountpoint])
            .checked_run()?;
        me.mounted = true;

        Ok(me)
    }
}

impl Drop for SnapMount {
    fn drop(&mut self) {
        if self.mounted {
            let st = Command::new("umount")
                .args(&[&self.mountpoint])
                .checked_run();
            match st {
                Err(e) => eprintln!("Error umounting: {:?}", e),
                Ok(()) => (),
            }
        }

        // Deactivate the volume.
        let st = Command::new("lvchange")
            .args(&["-an", "-K", &self.lvm_name])
            .checked_run();
        match st {
            Err(e) => eprintln!("Error running lvchange: {:?}", e),
            Ok(()) => (),
        }
    }
}

enum States {
    Sep,
    Name,
    BQuote,
    Value,
}

type Map = HashMap<String, String>;

fn parse(line: &str) -> Map {
    use self::States::*;

    let mut state = Sep;
    let mut key = String::new();
    let mut value = String::new();
    let mut result = Map::new();

    for ch in line.chars() {
        match state {
            Sep => {
                if ch != ' ' {
                    key.clear();
                    key.push(ch);
                    state = Name;
                }
            }
            Name => {
                if ch == '=' {
                    state = BQuote;
                } else {
                    key.push(ch);
                }
            }
            BQuote => {
                if ch == '\'' {
                    state = Value;
                    value.clear();
                } else {
                    panic!("Unexpected character");
                }
            }
            Value => {
                if ch == '\'' {
                    state = Sep;
                    result.insert(key.to_string(), value.to_string());
                } else {
                    value.push(ch);
                }
            }
        }
    }

    result
}
