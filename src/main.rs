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
        ).get_matches();
    // println!("matches: {:?}", matches);

    if matches.subcommand_matches("sync").is_some() {
        rack::sync_root().expect("sync root");
    } else if matches.subcommand_matches("snap").is_some() {
        rack::snapshot().expect("snapshot");
    } else {
        println!("Need to specify a command, try 'help'");
    }
}
