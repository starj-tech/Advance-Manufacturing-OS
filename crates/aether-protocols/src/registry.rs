use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// One mapping between a domain `tag` and its protocol source.
///
/// Loaded from `protocols.toml` per-site by the desktop app at startup.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub tag: String,
    /// e.g. `opcua://plc1/ns=2;s=Temperature` or `mqtt://broker/sensors/press_01/temp`
    pub source: String,
    pub unit: Option<String>,
    pub scale: Option<f64>,
    pub deadband: Option<f64>,
    #[serde(default)]
    pub writable: bool,
}

#[derive(Debug, Error)]
pub enum TagRegistryError {
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("duplicate tag: {0}")]
    Duplicate(String),
}

#[derive(Default, Debug, Clone)]
pub struct TagRegistry {
    by_tag: HashMap<String, Binding>,
}

#[derive(Debug, Deserialize)]
struct Doc {
    #[serde(default)]
    binding: Vec<Binding>,
}

impl TagRegistry {
    pub fn from_toml(input: &str) -> Result<Self, TagRegistryError> {
        let doc: Doc = toml::from_str(input)?;
        let mut by_tag = HashMap::new();
        for b in doc.binding {
            if by_tag.insert(b.tag.clone(), b.clone()).is_some() {
                return Err(TagRegistryError::Duplicate(b.tag));
            }
        }
        Ok(Self { by_tag })
    }

    pub fn lookup(&self, tag: &str) -> Option<&Binding> {
        self.by_tag.get(tag)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Binding)> {
        self.by_tag.iter()
    }

    pub fn len(&self) -> usize {
        self.by_tag.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_tag.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[[binding]]
tag = "press_01.temp"
source = "opcua://plc1/ns=2;s=Temperature"
unit = "celsius"
scale = 0.1
deadband = 0.5

[[binding]]
tag = "press_01.pressure"
source = "mqtt://broker/sensors/press_01/pressure"
unit = "bar"
"#;

    #[test]
    fn parses_two_bindings() {
        let reg = TagRegistry::from_toml(SAMPLE).unwrap();
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.lookup("press_01.temp").unwrap().unit.as_deref(), Some("celsius"));
        assert!(reg.lookup("missing").is_none());
    }

    #[test]
    fn duplicate_tag_is_rejected() {
        let dup = r#"
[[binding]]
tag = "x"
source = "a"

[[binding]]
tag = "x"
source = "b"
"#;
        assert!(matches!(
            TagRegistry::from_toml(dup),
            Err(TagRegistryError::Duplicate(_))
        ));
    }
}
