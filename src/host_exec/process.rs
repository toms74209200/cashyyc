use super::{Host, HostCmd};
use crate::err;
use crate::error::Result;

pub struct ProcessHost;

impl Host for ProcessHost {
    fn run_group(&mut self, cmds: &[HostCmd], label: &str) -> Result<bool> {
        let mut children = Vec::new();
        for cmd in cmds {
            let child = match cmd {
                HostCmd::Shell(s) => std::process::Command::new("sh").args(["-c", s]).spawn(),
                HostCmd::Exec(args) => std::process::Command::new(&args[0])
                    .args(&args[1..])
                    .spawn(),
            };
            children.push(child.map_err(|e| err!("Failed to run {label}: {e}"))?);
        }
        for child in &mut children {
            if !child
                .wait()
                .map_err(|e| err!("Failed to wait for {label}: {e}"))?
                .success()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn copy_dir(&mut self, src: &str, dest: &str) -> Result<bool> {
        std::process::Command::new("cp")
            .args(["-r", src, dest])
            .status()
            .map(|s| s.success())
            .map_err(|e| err!("failed to copy local feature: {e}"))
    }
}
