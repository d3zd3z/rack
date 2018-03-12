//! Program configuration.
//!
//! This module defines the config file.

use Result;
use serde_yaml;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    snap: SnapConfig,
    sure: SureConfig,
    restic: ResticConfig,
    clone: CloneConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapConfig {
    conventions: Vec<SnapConvention>,
    volumes: Vec<SnapVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapConvention {
    name: String,
    last: i32,
    hourly: i32,
    daily: i32,
    weekly: i32,
    monthly: i32,
    yearly: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapVolume {
    name: String,
    convention: String,
    zfs: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SureConfig {
    volumes: Vec<SureVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SureVolume {
    name: String,
    zfs: String,
    bind: String,
    sure: String,
    convention: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloneConfig {
    volumes: Vec<CloneVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloneVolume {
    name: String,
    source: String,
    dest: String,
    skip: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResticConfig {
    volumes: Vec<ResticVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResticVolume {
    name: String,
    zfs: String,
    bind: String,
    repo: String,
    passwordfile: String,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Config> {
        let fd = File::open(path)?;

        let item = serde_yaml::from_reader(fd)?;

        // TODO: Fixups?

        Ok(item)
    }
}
