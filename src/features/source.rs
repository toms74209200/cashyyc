use anyhow::{Result, anyhow};
use std::path::PathBuf;

pub enum FeatureSource {
    Local(PathBuf),
    Tarball(String),
    Oci {
        registry: String,
        path: String,
        version: String,
    },
}

impl FeatureSource {
    pub fn parse(id: &str) -> Result<Self> {
        if id.starts_with("./") || id.starts_with('/') {
            return Ok(Self::Local(PathBuf::from(id)));
        }
        if id.starts_with("https://") || id.starts_with("http://") {
            return Ok(Self::Tarball(id.to_string()));
        }
        let (without_version, version) = id
            .rsplit_once(':')
            .map(|(a, b)| (a, b.to_string()))
            .unwrap_or((id, "latest".to_string()));
        let slash = without_version
            .find('/')
            .ok_or_else(|| anyhow!("invalid OCI feature ref: {id}"))?;
        Ok(Self::Oci {
            registry: without_version[..slash].to_string(),
            path: without_version[slash + 1..].to_string(),
            version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
    use std::fs::File;

    fn urandom() -> File {
        File::open("/dev/urandom").unwrap()
    }

    fn random_name() -> String {
        generate_random_string(8, &[CharacterType::Lowercase], "", &mut urandom())
    }

    #[test]
    fn when_parse_with_oci_ref_then_parses_registry_path_and_version() {
        let registry = format!("{}.example.com", random_name());
        let path = format!("{}/{}", random_name(), random_name());
        let version = random_name();
        let id = format!("{registry}/{path}:{version}");

        let result = FeatureSource::parse(&id).unwrap();
        let FeatureSource::Oci {
            registry: r,
            path: p,
            version: v,
        } = result
        else {
            panic!("expected Oci variant");
        };
        assert_eq!(r, registry);
        assert_eq!(p, path);
        assert_eq!(v, version);
    }

    #[test]
    fn when_parse_with_oci_ref_without_version_then_version_is_latest() {
        let id = format!("{}/{}", random_name(), random_name());

        let result = FeatureSource::parse(&id).unwrap();
        let FeatureSource::Oci { version, .. } = result else {
            panic!("expected Oci variant");
        };
        assert_eq!(version, "latest");
    }

    #[test]
    fn when_parse_with_oci_ref_without_slash_then_returns_error() {
        let id = random_name();
        assert!(FeatureSource::parse(&id).is_err());
    }

    #[test]
    fn when_parse_with_https_url_then_preserves_url_as_tarball() {
        let url = format!("https://{}.example.com/feature.tgz", random_name());

        let result = FeatureSource::parse(&url).unwrap();
        let FeatureSource::Tarball(got) = result else {
            panic!("expected Tarball variant");
        };
        assert_eq!(got, url);
    }

    #[test]
    fn when_parse_with_http_url_then_preserves_url_as_tarball() {
        let url = format!("http://{}.example.com/feature.tgz", random_name());

        let result = FeatureSource::parse(&url).unwrap();
        let FeatureSource::Tarball(got) = result else {
            panic!("expected Tarball variant");
        };
        assert_eq!(got, url);
    }

    #[test]
    fn when_parse_with_relative_path_then_preserves_path_as_local() {
        let name = random_name();
        let id = format!("./{name}");

        let result = FeatureSource::parse(&id).unwrap();
        let FeatureSource::Local(path) = result else {
            panic!("expected Local variant");
        };
        assert_eq!(path, PathBuf::from(id));
    }

    #[test]
    fn when_parse_with_absolute_path_then_preserves_path_as_local() {
        let name = random_name();
        let id = format!("/{name}");

        let result = FeatureSource::parse(&id).unwrap();
        let FeatureSource::Local(path) = result else {
            panic!("expected Local variant");
        };
        assert_eq!(path, PathBuf::from(id));
    }
}
