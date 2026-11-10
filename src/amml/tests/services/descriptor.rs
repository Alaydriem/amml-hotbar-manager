use alaydriem_hotbar_manager::services::HotbarDescriptor;
use amm_core::config::ResolutionBook;
use amm_core::services::DescriptorChecker;
use amm_core::services::clash::ClashAnalyzer;
use amm_core::services::registry::ModRegistry;
use amm_core::structs::descriptor::{Capability, ClaimMode, Descriptor, KeyScope};

// the default chords: Alt+D rotates up, Alt+A rotates down, Alt+X sorts (virtual-key codes of D, A and X)
const DEFAULT_KEYS: [(&str, u32); 3] = [("rotate_up", 0x44), ("rotate_down", 0x41), ("sort", 0x58)];

fn descriptor() -> Descriptor {
    DescriptorChecker::new(128, 256).parse(HotbarDescriptor::TEXT).unwrap()
}

#[test]
fn descriptor_passes_manager_checker() {
    let descriptor = descriptor();

    assert_eq!(descriptor.namespace, "alaydriem");
    assert_eq!(descriptor.name, "hotbar-manager");
    assert_eq!(descriptor.id(), "@alaydriem/hotbar-manager");
    let claims: Vec<_> = descriptor.claims.iter().map(|claim| (claim.capability, claim.mode)).collect();
    assert_eq!(claims, [(Capability::Input, ClaimMode::Shared)]);
}

#[test]
fn the_three_keybinds_exist_with_alt_default_chords() {
    let descriptor = descriptor();

    let mut keybinds: Vec<_> = descriptor
        .keybinds
        .iter()
        .map(|keybind| (keybind.action.as_str(), keybind.capability, keybind.scope, keybind.config_path.as_str()))
        .collect();
    keybinds.sort_by_key(|keybind| keybind.0);
    assert_eq!(
        keybinds,
        [
            ("rotate_down", Capability::Input, KeyScope::Global, "keys.rotate_down"),
            ("rotate_up", Capability::Input, KeyScope::Global, "keys.rotate_up"),
            ("sort", Capability::Input, KeyScope::Global, "keys.sort"),
        ]
    );
    for (action, key) in DEFAULT_KEYS {
        let keybind = descriptor.keybinds.iter().find(|keybind| keybind.action == action).unwrap();
        let chord = keybind.default_chord;
        assert_eq!(chord.key, key, "{action} has another default key");
        assert!(chord.alt && !chord.ctrl && !chord.shift && !chord.f3, "{action} is not an Alt chord");
    }
}

#[test]
fn the_default_chords_report_no_clash_in_the_manager_analyzer() {
    let descriptor = descriptor();
    let mut registry = ModRegistry::new(16);
    let index = registry.register(descriptor, true).unwrap();
    let chords: Vec<_> = registry
        .get(index)
        .unwrap()
        .descriptor
        .keybinds
        .iter()
        .enumerate()
        .map(|(action, keybind)| (index, action as u16, keybind.default_chord))
        .collect();

    let mut analyzer = ClashAnalyzer::new();
    let report = analyzer.analyze(&registry, &ResolutionBook::new(), &chords);

    assert!(report.conflicts.is_empty(), "conflicts: {:?}", report.conflicts);
    assert!(report.interactions.is_empty());
}

#[test]
fn the_keys_section_has_a_chord_field_per_keybind() {
    let descriptor = descriptor();

    let keys = descriptor.settings.iter().find(|section| section.id == "keys").unwrap();
    for path in ["keys.rotate_up", "keys.rotate_down", "keys.sort"] {
        assert!(keys.fields.iter().any(|field| field.path == path), "{path} is missing");
    }
    assert_eq!(keys.fields.len(), 3);
}
