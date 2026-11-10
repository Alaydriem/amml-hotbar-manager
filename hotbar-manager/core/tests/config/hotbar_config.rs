use hotbar_manager_core::config::{HotbarConfig, KeyConfig};
use std::path::Path;

#[test]
fn missing_file_leaves_all_keys_unbound() {
    let config = HotbarConfig::read(Path::new("this/path/does/not/exist/config.hcl"));
    assert_eq!(config.keys, KeyConfig { rotate_up: None, rotate_down: None, sort: None });
}

#[test]
fn empty_text_leaves_all_keys_unbound() {
    assert_eq!(HotbarConfig::parse(""), HotbarConfig::default());
}

#[test]
fn sort_key_parses() {
    let config = HotbarConfig::parse("keys {\n  sort = \"R\"\n}\n");
    assert_eq!(config.keys.sort.as_deref(), Some("R"));
    assert_eq!(config.keys.rotate_up, None);
    assert_eq!(config.keys.rotate_down, None);
}

#[test]
fn all_three_keys_parse() {
    let config =
        HotbarConfig::parse("keys {\n  rotate_up = \"Ctrl+Up\"\n  rotate_down = \"Ctrl+Down\"\n  sort = \"R\"\n}\n");
    assert_eq!(config.keys.rotate_up.as_deref(), Some("Ctrl+Up"));
    assert_eq!(config.keys.rotate_down.as_deref(), Some("Ctrl+Down"));
    assert_eq!(config.keys.sort.as_deref(), Some("R"));
}

#[test]
fn reserved_placeholder_defaults_are_unbound() {
    let config =
        HotbarConfig::parse("keys {\n  rotate_up = \"VK_0x7\"\n  rotate_down = \"vk_0xa\"\n  sort = \"VK_0xB\"\n}\n");
    assert_eq!(config.keys, KeyConfig::default());
}

#[test]
fn unreadable_text_gives_defaults() {
    assert_eq!(HotbarConfig::parse("keys {{{"), HotbarConfig::default());
}

#[test]
fn zero_placeholder_is_unbound() {
    let config =
        HotbarConfig::parse("keys {\n  rotate_up = \"VK_0x0\"\n  rotate_down = \"vk_0x0\"\n  sort = \"R\"\n}\n");
    assert_eq!(config.keys.rotate_up, None);
    assert_eq!(config.keys.rotate_down, None);
    assert_eq!(config.keys.sort.as_deref(), Some("R"));
}

#[test]
fn wrong_type_key_only_drops_that_key() {
    let config = HotbarConfig::parse("keys {\n  rotate_up = \"Ctrl+Up\"\n  rotate_down = true\n  sort = 5\n}\n");
    assert_eq!(config.keys.rotate_up.as_deref(), Some("Ctrl+Up"));
    assert_eq!(config.keys.rotate_down, None);
    assert_eq!(config.keys.sort, None);
}

#[test]
fn unknown_field_and_extra_block_are_ignored() {
    let config = HotbarConfig::parse("toggles {\n  a = 1\n}\nkeys {\n  extra = \"x\"\n  sort = \"R\"\n}\n");
    assert_eq!(config.keys.sort.as_deref(), Some("R"));
    assert_eq!(config.keys.rotate_up, None);
}
