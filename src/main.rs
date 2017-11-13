#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]

#[macro_use] extern crate clap;
extern crate rack;

fn main() {
    let matches = clap_app!(
        myapp =>
        (version: "0.1")
        (author: "David Brown <davidb@davidb.org>")
        (about: "Snapshot based backups")
        (@arg PREFIX: -p --prefix +takes_value "Set snapshot prefix")
        (@subcommand sync =>
         (about: "rsync root volume to zfs volume")
         (@arg FS: --fs +takes_value "ZFS filesystem name (default lint/ext4gentoo)"))
        (@subcommand hsync =>
         (about: "rsync home volume to zfs volume")
         (@arg FS: --fs +takes_value "ZFS filesystem name (default lint/ext4home)"))
        (@subcommand snap =>
         (about: "take a current snapshot of concerned volumes")
         (@arg FS: --fs +takes_value "ZFS filesystem name (default lint/ext4gentoo)"))
        (@subcommand clone =>
         (about: "clone one volume tree to another")
         (@arg EXCLUDE: --exclude -e ... +takes_value "Tree to exclude (source based)")
         (@arg PRETEND: --pretend -n "Don't actually do the clone, but show what would be done")
         (@arg SOURCE: +required "Source zfs volume")
         (@arg DEST: +required "Destination zfs volume"))
        (@subcommand prune =>
         (about: "prune older snapshots")
         (@arg REALLY: --really "Actually do the prune")
         (@arg DEST: +required "Volume to prune"))
        (@subcommand sure =>
         (about: "Update rsure data")
         (@arg FS: --fs +takes_value "ZFS filesystem name")
         (@arg FILE: --file +takes_value "Weave file for rsure data"))
        (@subcommand borg =>
         (about: "Generate borg backups")
         (@arg FS: --fs +takes_value "ZFS filesystem name")
         (@arg REPO: --repo +takes_value "Borg repo path")
         (@arg NAME: --name +takes_value "Borg backup name prefix"))
        ).get_matches();

    let prefix = matches.value_of("PREFIX").unwrap_or("caz");

    if let Some(matches) = matches.subcommand_matches("sync") {
        let fs = matches.value_of("FS").unwrap_or("lint/ext4gentoo");
        rack::sync_root(fs).expect("sync root");
    } else if let Some(matches) = matches.subcommand_matches("hsync") {
        let fs = matches.value_of("FS").unwrap_or("lint/ext4home");
        rack::sync_home(fs).expect("sync home");
    } else if let Some(matches) = matches.subcommand_matches("snap") {
        let fs = matches.value_of("FS").unwrap_or("lint/ext4gentoo");
        rack::snapshot(prefix, fs).expect("snapshot");
    } else if let Some(matches) = matches.subcommand_matches("clone") {
        let source = matches.value_of("SOURCE").unwrap();
        let dest = matches.value_of("DEST").unwrap();
        let pretend = matches.is_present("PRETEND");
        let excludes: Vec<_> = matches.values_of("EXCLUDE").unwrap().collect();
        rack::clone(source, dest, !pretend, &excludes).expect("clone");
    } else if let Some(matches) = matches.subcommand_matches("prune") {
        let dest = matches.value_of("DEST").unwrap();
        let really = matches.is_present("REALLY");
        rack::prune(prefix, dest, really).expect("prune");
    } else if let Some(matches) = matches.subcommand_matches("sure") {
        let fs = matches.value_of("FS").unwrap_or("lint/ext4gentoo");
        let file = matches.value_of("FILE")
            .map(|x| x.to_string())
            .unwrap_or_else(|| format!("/lint/sure/ext4gentoo-{}.weave.gz", prefix));

        println!("Sure update of {:?} to {:?}", fs, file);
        rack::sure(prefix, fs, &file).expect("sure");
    } else if let Some(matches) = matches.subcommand_matches("borg") {
        let fs = matches.value_of("FS").unwrap_or("lint/ext4gentoo");
        let repo = matches.value_of("REPO").unwrap_or("/lint/borgs/linaro");
        let name = matches.value_of("NAME").unwrap_or("gentoo-");
        rack::run_borg(fs, repo, name).unwrap();
    } else {
        println!("Need to specify a command, try 'help'");
    }
}
