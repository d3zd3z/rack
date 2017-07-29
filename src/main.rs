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
        (@subcommand sync =>
         (about: "rsync root volume to zfs volume"))
        (@subcommand snap =>
         (about: "take a current snapshot of concerned volumes"))
        (@subcommand clone =>
         (about: "clone one volume tree to another")
         (@arg SOURCE: +required "Source zfs volume")
         (@arg DEST: +required "Destination zfs volume"))
        ).get_matches();
    // println!("matches: {:?}", matches);

    if matches.subcommand_matches("sync").is_some() {
        rack::sync_root().expect("sync root");
    } else if matches.subcommand_matches("snap").is_some() {
        rack::snapshot().expect("snapshot");
    } else if let Some(matches) = matches.subcommand_matches("clone") {
        let source = matches.value_of("SOURCE").unwrap();
        let dest = matches.value_of("DEST").unwrap();
        rack::clone(source, dest).expect("clone");
    } else {
        println!("Need to specify a command, try 'help'");
    }
}
