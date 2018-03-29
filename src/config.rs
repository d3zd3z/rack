//! Program configuration.
//!
//! This module defines the config file.

use Result;
use failure::err_msg;
use serde_yaml;
use std::env;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub snap: SnapConfig,
    pub sure: SureConfig,
    pub restic: ResticConfig,
    pub clone: CloneConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapConfig {
    pub conventions: Vec<SnapConvention>,
    pub volumes: Vec<SnapVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapConvention {
    pub name: String,
    pub last: Option<i32>,
    pub hourly: Option<i32>,
    pub daily: Option<i32>,
    pub weekly: Option<i32>,
    pub monthly: Option<i32>,
    pub yearly: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapVolume {
    pub name: String,
    pub convention: String,
    pub zfs: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SureConfig {
    pub volumes: Vec<SureVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SureVolume {
    pub name: String,
    pub zfs: String,
    pub bind: String,
    pub sure: String,
    pub convention: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloneConfig {
    pub volumes: Vec<CloneVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloneVolume {
    pub name: String,
    pub source: String,
    pub dest: String,
    pub skip: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResticConfig {
    pub volumes: Vec<ResticVolume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResticVolume {
    pub name: String,
    pub zfs: String,
    pub bind: String,
    pub repo: String,
    pub passwordfile: String,
}

impl Config {
    pub fn load_default() -> Result<Config> {
        let home = env::home_dir().ok_or_else(|| err_msg("Unable to find home directory"))?;
        let yaml = home.join(".gack.yaml");
        Config::load(yaml)
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Config> {
        let fd = File::open(path)?;

        let item = serde_yaml::from_reader(fd)?;

        // TODO: Fixups?

        Ok(item)
    }
}
