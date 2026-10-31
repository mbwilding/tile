//! `docs/configuration.md` must show the real defaults, so users can copy it as it is.

use std::{collections::BTreeSet, fs, path::Path};

use serde_yaml_ng::Value;
use tile_config::{Config, Loaded};

fn doc_block() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/configuration.md");
    let text = fs::read_to_string(path).expect("docs/configuration.md exists");
    let start = text
        .split("<!-- default-config:start -->")
        .nth(1)
        .expect("start marker");
    let block = start
        .split("<!-- default-config:end -->")
        .next()
        .expect("end marker");
    block
        .split("```yaml")
        .nth(1)
        .and_then(|b| b.split("```").next())
        .expect("a yaml fence")
        .to_string()
}

/// Every key path in a YAML value, so a setting missing from the docs is noticed even though
/// parsing would silently fill it in with its default. Unset (null) options don't count.
fn paths(value: &Value, prefix: &str, out: &mut BTreeSet<String>) {
    match value {
        Value::Mapping(map) => {
            for (key, value) in map.iter().filter(|(_, v)| !v.is_null()) {
                let key = key.as_str().expect("string keys");
                let path = format!("{prefix}.{key}");
                out.insert(path.clone());
                paths(value, &path, out);
            }
        }
        Value::Sequence(items) => {
            for item in items.iter().filter(|i| i.is_mapping()) {
                paths(item, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

#[test]
fn documented_defaults_are_the_real_defaults() {
    let block = doc_block();
    assert_eq!(Loaded::parse(&block).unwrap().config, Config::default());

    let documented: Value = serde_yaml_ng::from_str(&block).unwrap();
    let real = serde_yaml_ng::to_value(Config::default()).unwrap();
    let (mut a, mut b) = (BTreeSet::new(), BTreeSet::new());
    paths(&documented, "", &mut a);
    paths(&real, "", &mut b);
    assert_eq!(
        a.difference(&b).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "documented but not a setting"
    );
    assert_eq!(
        b.difference(&a).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "a setting missing from docs/configuration.md"
    );
}
