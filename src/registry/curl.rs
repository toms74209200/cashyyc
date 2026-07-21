use super::{Fetched, Registry};
use crate::err;
use crate::error::Result;

pub struct CurlRegistry;

const MANIFEST_ACCEPT: &str = "Accept: application/vnd.oci.image.manifest.v1+json";

impl CurlRegistry {
    fn output(&mut self, args: &[&str]) -> Result<Fetched> {
        let output = std::process::Command::new("curl")
            .args(args)
            .output()
            .map_err(|e| err!("failed to run curl: {e}"))?;
        Ok(Fetched {
            success: output.status.success(),
            body: output.stdout,
        })
    }

    fn status(&mut self, args: &[&str]) -> Result<bool> {
        std::process::Command::new("curl")
            .args(args)
            .status()
            .map(|s| s.success())
            .map_err(|e| err!("failed to run curl: {e}"))
    }
}

impl Registry for CurlRegistry {
    fn fetch(&mut self, url: &str) -> Result<Fetched> {
        self.output(&["-sf", url])
    }

    fn fetch_manifest(&mut self, url: &str, token: &str) -> Result<Fetched> {
        let auth = format!("Authorization: Bearer {token}");
        self.output(&["-sf", "-H", &auth, "-H", MANIFEST_ACCEPT, url])
    }

    fn fetch_blob(&mut self, url: &str, token: &str) -> Result<Fetched> {
        let auth = format!("Authorization: Bearer {token}");
        self.output(&["-sfL", "-H", &auth, url])
    }

    fn download_blob(&mut self, url: &str, token: &str, dest: &str) -> Result<bool> {
        let auth = format!("Authorization: Bearer {token}");
        self.status(&["-sfL", "-H", &auth, "-o", dest, url])
    }

    fn download(&mut self, url: &str, dest: &str) -> Result<bool> {
        self.status(&["-sfL", url, "-o", dest])
    }

    fn unpack_tar(&mut self, archive: &str, dest: &str) -> Result<bool> {
        std::process::Command::new("tar")
            .args(["xf", archive, "-C", dest])
            .status()
            .map(|s| s.success())
            .map_err(|e| err!("failed to run tar: {e}"))
    }
}
