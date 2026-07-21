use crate::error::Error;

pub struct Fetched {
    pub success: bool,
    pub body: Vec<u8>,
}

pub trait Registry {
    fn fetch(&mut self, url: &str) -> Result<Fetched, Error>;
    fn fetch_manifest(&mut self, url: &str, token: &str) -> Result<Fetched, Error>;
    fn fetch_blob(&mut self, url: &str, token: &str) -> Result<Fetched, Error>;
    fn download_blob(&mut self, url: &str, token: &str, dest: &str) -> Result<bool, Error>;
    fn download(&mut self, url: &str, dest: &str) -> Result<bool, Error>;
    fn unpack_tar(&mut self, archive: &str, dest: &str) -> Result<bool, Error>;
}
