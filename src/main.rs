#[macro_use] extern crate clap;
extern crate rack;

fn main() {
    let matches = clap_app!(
        myapp =>
        (version: "0.1")
        (author: "David Brown <davidb@davidb.org>")
        (about: "Snapshot based backups")
        (@subcommand sync =>
         (about: "rsync root volume to zfs volume")
        )).get_matches();
    // println!("matches: {:?}", matches);

    if let Some(_) = matches.subcommand_matches("sync") {
        rack::sync_root().expect("sync root");
    }
}
