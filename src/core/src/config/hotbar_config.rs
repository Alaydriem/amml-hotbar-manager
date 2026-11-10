use serde::Deserialize;
use std::path::Path;

// the descriptor's reserved key codes (0x07, 0x0A, 0x0B) stand for "no key"; "VK_0x0" comes from an earlier descriptor
const UNBOUND: [&str; 4] = ["VK_0x0", "VK_0x7", "VK_0xA", "VK_0xB"];

/// The chord text of each key setting; `None` means the key is not bound.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyConfig {
    pub rotate_up: Option<String>,
    pub rotate_down: Option<String>,
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HotbarConfig {
    pub keys: KeyConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawSettings {
    keys: RawKeys,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawKeys {
    rotate_up: Option<hcl::Value>,
    rotate_down: Option<hcl::Value>,
    sort: Option<hcl::Value>,
}

impl HotbarConfig {
    /// Reads `config.hcl` text. Text that cannot be read gives the defaults (all keys unbound); the caller should log it.
    /// A key whose value is not text is unbound; the other keys still count.
    pub fn parse(text: &str) -> Self {
        match hcl::from_str::<RawSettings>(text) {
            Ok(raw) => Self {
                keys: KeyConfig {
                    rotate_up: Self::bound(raw.keys.rotate_up),
                    rotate_down: Self::bound(raw.keys.rotate_down),
                    sort: Self::bound(raw.keys.sort),
                },
            },
            Err(_) => Self::default(),
        }
    }

    /// Reads a config file. A missing or unreadable file gives the defaults.
    pub fn read(path: &Path) -> Self {
        std::fs::read_to_string(path).map(|text| Self::parse(&text)).unwrap_or_default()
    }

    // a value that is not text counts as unbound; it does not affect the other keys
    fn bound(value: Option<hcl::Value>) -> Option<String> {
        let text = match value {
            Some(hcl::Value::String(text)) => Some(text),
            _ => None,
        };
        text.map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty() && !UNBOUND.iter().any(|reserved| reserved.eq_ignore_ascii_case(text)))
    }
}
