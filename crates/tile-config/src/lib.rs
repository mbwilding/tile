//! TOML configuration for Tile: schema, keybind parsing, window rules and hot reload.

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

use tile_core::Command;

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
        Self::from_config(toml::from_str(text)?)
    }

    /// The built-in defaults, used when no config file exists.
    pub fn defaults() -> Self {
        Self::from_config(Config::default()).expect("built-in defaults are valid")
    }

    /// Loads `path`, or the built-in defaults when the file doesn't exist.
    pub fn load_or_default(path: &Path) -> Result<Self, ConfigError> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::defaults())
        }
    }

    fn from_config(config: Config) -> Result<Self, ConfigError> {
        let binds = config
            .binds
            .iter()
            .map(|(keys, action)| {
                let bind = keys
                    .parse::<Keybind>()
                    .map_err(|e| ConfigError::Bind(keys.clone(), e))?;
                let command = action
                    .parse::<Command>()
                    .map_err(|e| ConfigError::Bind(keys.clone(), e.to_string()))?;
                Ok((bind, command))
            })
            .collect::<Result<_, ConfigError>>()?;
        let rules = Rules::compile(&config.rules)?;
        Ok(Self {
            config,
            binds,
            rules,
        })
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|e| ConfigError::Io(path.into(), e))?;
        Self::parse(&text)
    }
}

/// `%APPDATA%\Tile\config.toml`. Created on first run with every default commented out.
pub fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Tile")
        .join("config.toml")
}

const TEMPLATE_HEADER: &str = "\
# Tile config. Everything below is the built-in default, commented out.
# Uncomment a line (and its [section] header) to override it; saved changes are applied live.
# A [binds] or [[rules]] table replaces that whole set of defaults.

";

/// The built-in defaults as TOML, every line commented out so the file changes nothing until
/// a line is uncommented.
pub fn template() -> String {
    let defaults = toml::to_string(&Config::default()).expect("built-in defaults serialize");
    let mut text = String::from(TEMPLATE_HEADER);
    for line in defaults.lines() {
        if !line.is_empty() {
            text.push_str("# ");
        }
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// Creates the config's directory, and the file from [`template`] if it doesn't exist yet.
pub fn ensure_exists(path: &Path) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        // Don't leave a partial file behind: it would stop the template being written again.
        Ok(mut file) => io::Write::write_all(&mut file, template().as_bytes()).inspect_err(|_| {
            drop(file);
            let _ = fs::remove_file(path);
        }),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_defaults_are_valid() {
        let loaded = Loaded::defaults();
        assert!(loaded.binds.len() > 40);
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn empty_config_uses_defaults() {
        let loaded = Loaded::parse("").unwrap();
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn missing_file_uses_defaults() {
        let loaded = Loaded::load_or_default(Path::new("does/not/exist.toml")).unwrap();
        assert_eq!(loaded.config, Config::default());
    }

    #[test]
    fn template_is_inert_and_uncomments_to_defaults() {
        let template = template();
        assert_eq!(Loaded::parse(&template).unwrap().config, Config::default());
        let uncommented: String = template[TEMPLATE_HEADER.len()..]
            .lines()
            .map(|l| format!("{}\n", l.strip_prefix("# ").unwrap_or(l)))
            .collect();
        assert_eq!(
            Loaded::parse(&uncommented).unwrap().config,
            Config::default()
        );
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
    fn win_l_bind_loads_without_override() {
        // The daemon skips it instead, so the default binds stay valid either way.
        let config = "[general]\noverride_win_l = false\n[binds]\n\"super+l\" = \"movefocus r\"\n";
        assert!(Loaded::parse(config).unwrap().binds[0].0.is_win_l());
    }

    #[test]
    fn rejects_unknown_fields() {
        assert!(Loaded::parse("[general]\ngap = 2\n").is_err());
    }
}
