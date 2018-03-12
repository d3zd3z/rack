#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

extern crate rack;

#[macro_use] extern crate structopt_derive;
extern crate structopt;

use std::env;
use std::process;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "rack", about = "Snapshot based backups")]
struct Opt {
    #[structopt(short = "p", long = "prefix", default_value = "caz")]
    prefix: String,
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
        #[structopt(long = "fs", default_value = "lint/ext4gentoo")]
        /// ZFS filesystem name
        fs: String,
    },

    #[structopt(name = "clone")]
    /// Clone one volume tree to another
    CloneCmd {
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
        #[structopt(long = "fs", default_value = "lint/ext4gentoo")]
        /// ZFS filesystem name
        fs: String,

        #[structopt(long = "file")]
        /// Weave file for rsure data
        file: Option<String>,
    },

    #[structopt(name = "borg")]
    /// Generate borg backups
    Borg {
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

    #[structopt(name = "hack")]
    /// Hacking work for new api.
    Hack,
}

fn main() {
    match main_err() {
        Ok(()) => (),
        Err(e) => {
            println!("Error: {}", e);
            if env::var("RUST_BACKTRACE").is_ok() {
                println!("{}", e.backtrace());
            } else {
                println!("Run with RUST_BACKTRACE=1 for a backtrace");
            }
            process::exit(1);
        }
    }
}

fn main_err() -> rack::Result<()> {
    let opt = Opt::from_args();

    match opt.command {
        Command::SyncCmd { fs } => {
            rack::sync_root(&fs)?;
        }
        Command::HSync { fs } => {
            rack::sync_home(&fs)?;
        }
        Command::Snap { fs } => {
            rack::snapshot(&opt.prefix, &fs)?;
        }
        Command::CloneCmd { excludes, pretend, source, dest } => {
            let excl: Vec<_> = excludes.iter().map(|x| x.as_str()).collect();
            rack::clone(&source, &dest, !pretend, &excl)?;
        }
        Command::Prune { really, dest } => {
            rack::prune(&opt.prefix, &dest, really)?;
        }
        Command::Sure { fs, file } => {
            let file = match file {
                Some(f) => f,
                None => format!("/lint/sure/ext4gentoo-{}.weave.gz", opt.prefix),
            };
            println!("Sure update of {:?} to {:?}", fs, file);
            rack::sure(&opt.prefix, &fs, &file)?;
        }
        Command::Borg { fs, repo, name } => {
            rack::run_borg(&fs, &repo, &name)?;
        }
        Command::Hack => {
            let conf = rack::Config::load("/home/davidb/.gack.yaml")?;
            println!("Config file: {:?}", conf);
        }
    }
    Ok(())
}
