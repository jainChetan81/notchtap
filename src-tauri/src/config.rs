use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    pub default_ttl: u64,
    pub max_concurrent: usize,
    pub max_queued: usize,
    pub detect_path: PathBuf,
}

fn default_port() -> u16 {
    9789
}

fn default_ttl() -> u64 {
    8
}

fn default_max_concurrent() -> usize {
    3
}

fn default_max_queued() -> usize {
    50
}

fn default_detect_path() -> PathBuf {
    PathBuf::from("/usr/local/bin/notchtap-detect")
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: default_port(),
            default_ttl: default_ttl(),
            max_concurrent: default_max_concurrent(),
            max_queued: default_max_queued(),
            detect_path: default_detect_path(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        // spec §9 pins ~/.config/notchtap/config.toml. dirs::config_dir()
        // is wrong here: on macOS it resolves to ~/Library/Application Support.
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
        let path = home.join(".config").join("notchtap").join("config.toml");

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("failed to read config at {:?}: {}", path, e))?;
        Self::parse(&content).map_err(|e| anyhow::anyhow!("failed to parse config at {:?}: {}", path, e))
    }

    pub fn parse(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toml_yields_all_defaults() {
        let c = Config::parse("").unwrap();
        assert_eq!(c.port, 9789);
        assert_eq!(c.default_ttl, 8);
        assert_eq!(c.max_concurrent, 3);
        assert_eq!(c.max_queued, 50);
        assert_eq!(c.detect_path, PathBuf::from("/usr/local/bin/notchtap-detect"));
    }

    #[test]
    fn partial_toml_keeps_defaults_for_missing_fields() {
        let c = Config::parse("port = 1234\n").unwrap();
        assert_eq!(c.port, 1234);
        assert_eq!(c.default_ttl, 8);
        assert_eq!(c.max_queued, 50);
    }

    #[test]
    fn malformed_toml_is_an_error() {
        assert!(Config::parse("port = \"not a number\"").is_err());
    }
}
