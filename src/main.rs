#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

use rack;

use chrono::Utc;
use std::path::Path;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "rack", about = "Snapshot based backups")]
struct Opt {
    #[structopt(short = "p", long = "prefix", default_value = "caz")]
    prefix: String,
    /// Override default config file.  Default ~/.gack.yaml.
    #[structopt(long = "config")]
    config: Option<String>,
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "sync")]
    /// rsync root volume to zfs volume
    SyncCmd {
        #[structopt(long = "fs", default_value = "lint/ext4gentoo")]
        /// ZFS filesystem name
        fs: String,
    },

    #[structopt(name = "hsync")]
    /// rsync home volume to zfs volume
    HSync {
        #[structopt(long = "fs", default_value = "lint/ext4home")]
        /// ZFS filesystem name
        fs: String,
    },

    #[structopt(name = "snap")]
    /// Take a current snapshot of concerned volumes.
    Snap {
        #[structopt(short = "n", long = "pretend")]
        /// show what would be executed, but don't actually run.
        pretend: bool,
    },

    #[structopt(name = "cloneone")]
    /// Clone one volume tree to another.  With explicit arguments
    CloneOneCmd {
        #[structopt(short = "e", long = "exclude")]
        /// Tree(s) to exclude (source based)
        excludes: Vec<String>,

        #[structopt(short = "n", long = "pretend")]
        /// Don't actually do the clone, but show what would be done
        pretend: bool,

        /// Source zfs filesystem
        source: String,

        /// Destination zfs filesystem
        dest: String,
    },

    #[structopt(name = "clone")]
    /// Clone/sync any filesystems as described in the config file.
    CloneCmd {
        #[structopt(short = "n", long = "pretend")]
        /// Don't actually do the work, just show what would be done
        pretend: bool,
    },

    #[structopt(name = "prune")]
    /// Prune older snapshots
    Prune {
        #[structopt(long = "really")]
        /// Actually do the prune
        really: bool,

        /// Volume to prune
        dest: String,
    },

    #[structopt(name = "sure")]
    /// Update rsure data
    Sure {
        /// Don't actually do the operation, but show what would be done.
        #[structopt(short = "n", long = "pretend")]
        pretend: bool,
    },

    #[structopt(name = "borg")]
    /// Generate borg backups
    Borg {
        #[structopt(short = "n", long = "pretend")]
        /// Don't actually do the backups, but show what would be done.
        pretend: bool,

        #[structopt(long = "fs", default_value = "lint/ext4gentoo")]
        /// ZFS filesystem name
        fs: String,

        #[structopt(long = "repo", default_value = "/lint/borgs/linaro")]
        /// Borg repo path
        repo: String,

        #[structopt(long = "name", default_value = "gentoo-")]
        /// Borg backup name prefix
        name: String,
    },

    #[structopt(name = "restic")]
    /// Generate restic backups.
    Restic {
        #[structopt(short = "n", long = "pretend")]
        /// Don't actually do the backups, but show what would be done.
        pretend: bool,

        #[structopt(long = "name")]
        /// Volume from .gack.yaml to back up.
        name: Option<String>,

        #[structopt(long = "limit")]
        /// Limit how many backups are made.
        limit: Option<usize>,
    },

    #[structopt(name = "hack")]
    /// Hacking work for new api.
    Hack,
}

fn main() -> rack::Result<()> {
    rsure::log_init();

    let opt = Opt::from_args();

    let config_file = opt.config.as_ref().map_or_else(
        || rack::Config::get_default(),
        |c| Ok(Path::new(c).to_path_buf()),
    )?;

    match opt.command {
        Command::SyncCmd { fs } => {
            rack::sync_root(&fs)?;
        }
        Command::HSync { fs } => {
            rack::sync_home(&fs)?;
        }
        Command::Snap { pretend } => {
            let conf = rack::Config::load(&config_file)?;
            conf.snap.snapshot(Utc::now(), pretend)?;
        }
        Command::CloneOneCmd {
            excludes,
            pretend,
            source,
            dest,
        } => {
            let excl: Vec<_> = excludes.iter().map(|x| x.as_str()).collect();
            rack::clone(&source, &dest, !pretend, &excl)?;
        }
        Command::CloneCmd { pretend } => {
            let conf = rack::Config::load(&config_file)?;
            conf.clone.run(pretend)?;
        }
        Command::Prune { really, dest } => {
            rack::prune(&opt.prefix, &dest, really)?;
        }
        Command::Sure { pretend } => {
            let conf = rack::Config::load(&config_file)?;
            conf.sure.run(pretend)?;
        }
        Command::Borg { fs, repo, name, pretend } => {
            rack::run_borg(&fs, &repo, &name, pretend)?;
        }
        Command::Restic { name, pretend, limit } => {
            let conf = rack::Config::load(&config_file)?;
            conf.run_restic(name.as_ref().map(|s| s.as_str()), limit, pretend)?;
        }
        Command::Hack => {
            let conf = rack::Config::load_default()?;
            println!("Config file: {:?}", conf);
        }
    }
    Ok(())
}
