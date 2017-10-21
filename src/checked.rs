//! An extension to Command to allow checked runs.

use std::process::Command;
use {ErrorKind, Result};

pub trait CheckedExt {
    /// Run the given command, normalizing to the local Result type, and returning a local error if
    /// the command doesn't return success.
    fn checked_run(&mut self) -> Result<()>;
}

impl CheckedExt for Command {
    fn checked_run(&mut self) -> Result<()> {
        let status = self.status()?;
        if !status.success() {
            return Err(ErrorKind::Command(format!("{:?}", self), status).into());
        }
        Ok(())
    }
}
