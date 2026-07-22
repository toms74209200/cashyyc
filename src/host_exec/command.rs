use crate::error::Error;

pub enum HostCmd {
    Shell(String),
    Exec(Vec<String>),
}

pub trait Host {
    fn run_group(&mut self, cmds: &[HostCmd], label: &str) -> Result<bool, Error>;
    fn copy_dir(&mut self, src: &str, dest: &str) -> Result<bool, Error>;
}
