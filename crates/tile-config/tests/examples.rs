//! The configs in `examples/` must load, and `default.yml` must be the real defaults.

use std::{collections::HashSet, fs, path::Path};

use tile_config::{Config, General, Loaded};

fn examples() -> impl Iterator<Item = std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    fs::read_dir(dir)
        .expect("examples/ exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "yml" || e == "yaml"))
}

#[test]
fn examples_load() {
    for path in examples() {
        Loaded::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}

#[test]
fn default_example_is_the_defaults() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/default.yml");
    let example = Loaded::load(&path).unwrap();
    let defaults = Loaded::defaults();

    let binds = |l: &Loaded| -> HashSet<String> {
        l.binds.iter().map(|(k, c)| format!("{k} {c}")).collect()
    };
    assert_eq!(binds(&example), binds(&defaults), "default.yml binds");

    let config = Config {
        general: General {
            default_binds: true,
            ..example.config.general.clone()
        },
        binds: Default::default(),
        ..example.config
    };
    assert_eq!(config, Config::default(), "default.yml settings");
}
