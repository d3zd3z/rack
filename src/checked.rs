//! An extension to Command to allow checked runs.

use std::process::{Command, Output};
use {ErrorKind, Result};

pub trait CheckedExt {
    /// Run the given command, normalizing to the local Result type, and returning a local error if
    /// the command doesn't return success.
    fn checked_run(&mut self) -> Result<()>;

    /// Run command, collecting all of its output.  Runs Command's `output` method, with an
    /// additional check of the status result.
    fn checked_output(&mut self) -> Result<Output>;
}

impl CheckedExt for Command {
    fn checked_run(&mut self) -> Result<()> {
        let status = self.status()?;
        if !status.success() {
            return Err(ErrorKind::Command(format!("{:?}", self), status).into());
        }
        Ok(())
    }

    fn checked_output(&mut self) -> Result<Output> {
        let out = self.output()?;
        if !out.status.success() {
            return Err(ErrorKind::Command(format!("{:?}", self), out.status).into());
        }
        Ok(out)
    }
}
