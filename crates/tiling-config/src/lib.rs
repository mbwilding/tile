//! TOML configuration for `tiling`: schema, keybind parsing, window rules and hot reload.

mod keys;
mod rules;
mod schema;
mod watch;

use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub use keys::{Keybind, Modifiers};
pub use rules::{RuleOutcome, Rules};
pub use schema::{Animations, Color, Config, General, Rule};
pub use watch::watch;

use tiling_core::Command;

/// The configuration shipped with the binary.
pub const DEFAULT_TOML: &str = include_str!("../../../config/default.toml");

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read {0}: {1}")]
    Io(PathBuf, io::Error),
    #[error("invalid config: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("bind `{0}`: {1}")]
    Bind(String, String),
    #[error("rule #{0}: {1}")]
    Rule(usize, String),
}

/// A validated configuration, ready to use.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub config: Config,
    pub binds: Vec<(Keybind, Command)>,
    pub rules: Rules,
}

impl Loaded {
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(text)?;
        let binds = config
            .binds
            .iter()
            .map(|(keys, action)| {
                let bind = keys.parse::<Keybind>().map_err(|e| ConfigError::Bind(keys.clone(), e))?;
                let command = action
                    .parse::<Command>()
                    .map_err(|e| ConfigError::Bind(keys.clone(), e.to_string()))?;
                Ok((bind, command))
            })
            .collect::<Result<_, ConfigError>>()?;
        let rules = Rules::compile(&config.rules)?;
        Ok(Self { config, binds, rules })
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|e| ConfigError::Io(path.into(), e))?;
        Self::parse(&text)
    }
}

/// `%APPDATA%\tiling\config.toml`.
pub fn default_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("tiling").join("config.toml")
}

/// Writes the default config to `path` if nothing exists there yet.
pub fn ensure_exists(path: &Path) -> io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, DEFAULT_TOML)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid_and_matches_struct_defaults() {
        let loaded = Loaded::parse(DEFAULT_TOML).unwrap();
        assert!(loaded.binds.len() > 40);
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn empty_config_uses_defaults() {
        let loaded = Loaded::parse("").unwrap();
        assert_eq!(loaded.binds.len(), Loaded::parse(DEFAULT_TOML).unwrap().binds.len());
    }

    #[test]
    fn partial_section_keeps_other_defaults() {
        let loaded = Loaded::parse("[general]\ngaps_in = 2\n").unwrap();
        assert_eq!(loaded.config.general.gaps_in, 2);
        assert_eq!(loaded.config.general.gaps_out, General::default().gaps_out);
    }

    #[test]
    fn reports_bad_binds() {
        let err = Loaded::parse("[binds]\n\"super+q\" = \"explode\"\n").unwrap_err();
        assert!(err.to_string().contains("super+q"));
        assert!(Loaded::parse("[binds]\n\"hyper+q\" = \"killactive\"\n").is_err());
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(Loaded::parse("[general]\ngap = 2\n").is_err());
    }
}
