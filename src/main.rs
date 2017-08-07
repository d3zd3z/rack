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
         (about: "rsync root volume to zfs volume"))
        (@subcommand snap =>
         (about: "take a current snapshot of concerned volumes"))
        (@subcommand clone =>
         (about: "clone one volume tree to another")
         (@arg SOURCE: +required "Source zfs volume")
         (@arg DEST: +required "Destination zfs volume"))
        (@subcommand prune =>
         (about: "prune older snapshots")
         (@arg REALLY: --really "Actually do the prune")
         (@arg DEST: +required "Volume to prune"))
        (@subcommand sure =>
         (about: "Update rsure data")
         (@arg FS: --fs "ZFS filesystem name")
         (@arg FILE: --file "Weave file for rsure data"))
        ).get_matches();

    let prefix = matches.value_of("PREFIX").unwrap_or("caz");

    if matches.subcommand_matches("sync").is_some() {
        rack::sync_root().expect("sync root");
    } else if matches.subcommand_matches("snap").is_some() {
        rack::snapshot(prefix).expect("snapshot");
    } else if let Some(matches) = matches.subcommand_matches("clone") {
        let source = matches.value_of("SOURCE").unwrap();
        let dest = matches.value_of("DEST").unwrap();
        rack::clone(source, dest).expect("clone");
    } else if let Some(matches) = matches.subcommand_matches("prune") {
        let dest = matches.value_of("DEST").unwrap();
        let really = matches.is_present("REALLY");
        rack::prune(prefix, dest, really).expect("prune");
    } else if let Some(matches) = matches.subcommand_matches("sure") {
        let fs = matches.value_of("FS").unwrap_or("lint/ext4root");
        let file = matches.value_of("FILE")
            .map(|x| x.to_string())
            .unwrap_or_else(|| format!("/lint/sure/ext4root-{}.weave.gz", prefix));

        println!("Sure update of {:?} to {:?}", fs, file);
        rack::sure(prefix, fs, &file).expect("sure");
    } else {
        println!("Need to specify a command, try 'help'");
    }
}
