# Rack - Rust backup utilities

This crate contains a utility program `rack` that I use to perform
various snapshot and cloning operations using ZFS filesystems.  It
takes various subcommands.  Most of these commands will need to run as
root.

## Commands

All commands are given as a argument to `rack`.  `rack` itself can
take the `--prefix` argument to override the default prefix used by
several commands.

### Sync

The `rack sync` command, is used to rsync the data from my root
filesystem to a specific ZFS volume.  Currently, all of the values are
hardcoded.  It does something like:

```
mount --bind / /mnt/root
rsync -aiHAx --delete /. /mnt/root/.
umount --bind /mnt/root
```

In this future, this will take more arguments to be a little more
flexible.  The bind mount is used so that the backup can back up files
that may be hid under mountpoints.  Note that Linux seems to put newly
mounted volumes inside of the bind mount (mounting them twice), which
can cause errors on the unmount (and the rsync to do too much work).

### Snap

The `rack snap` command creates a snapshot of specific volumes.
Currently, the parameters are hard coded, both the name and format of
the snapshots, and the volume the snapshot is taken on.

The `--prefix` argument can be given to `rack` to set the prefix on
the snapshots.  The default is currently "caz", which is meaningless.

### Prune

To keep snapshots from growing excessively, the `rack prune` command
can be used to remove some older snapshots.  Currently, it takes and
argument of the volume to prune, and will print out which snapshots it
would remove.  You can set `--prefix` to prune from a different
prefix, but the name format must match that done by `snap` above.

To really prune snapshots, pass the `--really` argument.

### Clone

`rack clone` takes two arguments, a source and a destination, which
name filesystems, and clones all of the snapshots from the source, and
its children into the destination and its children.  Child filesystems
will be created in the destination as needed, but only one level will
be created at any time (for example a destination of `foo/bar/baz`
will work if there is a `foo/bar` but not yet `foo/bar/baz`, but will
not work if only `foo` exists).

When creating filesystems, `rack clone` reads the ZFS properties from
the source volume, and will set any that have "local" or "received"
values on the destination.  However, it will always ignore the
"mountpoint" option, to avoid confusion of the destination volumes
trying to be mounted on top of existing volumes.  A future command
will have support for capturing mountpoints of filesystems and
restoring them if necessary.

## License

Licensed under

 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
