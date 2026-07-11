//! TI-6 string table. All player-visible strings live in
//! assets/locales/en-US.ron; code refers to them by dotted key.
//! EFIGS hook: swap the include for a boot-time locale file load.

use std::collections::HashMap;
use std::sync::LazyLock;

static STRINGS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    ron::from_str(include_str!("../assets/locales/en-US.ron"))
        .expect("Failed to parse en-US.ron")
});

/// Look up a UI string. A missing key returns the key itself so the
/// gap is visible in-game instead of panicking.
pub fn tr(key: &'static str) -> &'static str {
    match STRINGS.get(key) {
        Some(s) => s.as_str(),
        None => {
            bevy::log::warn!("missing string key: {key}");
            key
        }
    }
}

/// `tr` plus `{placeholder}` interpolation.
pub fn trf(key: &'static str, args: &[(&str, &str)]) -> String {
    let mut s = tr(key).to_string();
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_parses_and_lookup_works() {
        assert_eq!(tr("test.probe"), "probe value");
    }

    #[test]
    fn missing_key_returns_key() {
        assert_eq!(tr("no.such.key"), "no.such.key");
    }

    #[test]
    fn trf_interpolates() {
        assert_eq!(
            trf("test.greet", &[("name", "Sera"), ("n", "3")]),
            "Hello Sera, you have 3 quests"
        );
    }

    /// Every tr()/trf() key literal in src/ must exist in en-US.ron and
    /// vice versa. Dynamically built keys are disallowed.
    #[test]
    fn coverage_set_equality() {
        let keys_in_code = scan_src_for_keys();
        let keys_in_table: std::collections::BTreeSet<String> = STRINGS.keys().cloned().collect();
        let missing: Vec<_> = keys_in_code.difference(&keys_in_table).collect();
        let orphaned: Vec<_> = keys_in_table
            .difference(&keys_in_code)
            .filter(|k| !k.starts_with("test."))
            .collect();
        assert!(
            missing.is_empty(),
            "keys used in code but absent from en-US.ron: {missing:?}"
        );
        assert!(
            orphaned.is_empty(),
            "orphan keys in en-US.ron never used in code: {orphaned:?}"
        );
    }

    fn scan_src_for_keys() -> std::collections::BTreeSet<String> {
        let re = regex::Regex::new(r#"\btrf?\(\s*"([a-z0-9_.]+)""#).unwrap();
        let mut keys = std::collections::BTreeSet::new();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for cap in re.captures_iter(&text) {
                        keys.insert(cap[1].to_string());
                    }
                }
            }
        }
        // This test module's own literals.
        keys.remove("test.probe");
        keys.remove("test.greet");
        keys.remove("no.such.key");
        keys
    }
}
