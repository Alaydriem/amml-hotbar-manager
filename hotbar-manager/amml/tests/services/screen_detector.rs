use alaydriem_hotbar_manager::services::ScreenDetector;
use alaydriem_hotbar_manager::types::{ScreenCalls, UiProfile};
use hotbar_manager_core::structs::ScreenKind;

use crate::support::fake_host::FakeHost;

#[test]
fn each_root_name_of_the_spike_panel_report_gives_its_screen_kind() {
    let cases = [
        ("small_chest_screen", ScreenKind::SmallChest),
        ("large_chest_screen", ScreenKind::LargeChest),
        ("barrel_screen", ScreenKind::Barrel),
        ("shulker_box_screen", ScreenKind::ShulkerBox),
        ("ender_chest_screen", ScreenKind::EnderChest),
        ("hopper_screen", ScreenKind::Hopper),
        ("dispenser_screen", ScreenKind::Dispenser),
        ("dropper_screen", ScreenKind::Dropper),
        ("inventory_screen", ScreenKind::PlayerInventory),
        ("inventory_screen_pocket", ScreenKind::PlayerInventory),
    ];

    for (name, kind) in cases {
        assert_eq!(ScreenDetector::kind_of(name), kind, "{name}");
    }
}

#[test]
fn the_other_screens_of_the_report_and_unknown_names_give_other() {
    let names = [
        "start_screen",
        "debug_screen",
        "toast_screen",
        "hud_crosshair_screen",
        "chat_screen",
        "world_loading_progress_screen",
        "crafting_screen",
        "",
        "Small_Chest_Screen",
        "small_chest_screen ",
    ];

    for name in names {
        assert_eq!(ScreenDetector::kind_of(name), ScreenKind::Other, "{name:?}");
    }
}

#[test]
fn the_profile_value_zero_is_classic_and_one_is_pocket() {
    assert_eq!(UiProfile::from_value(0), UiProfile::Classic);
    assert_eq!(UiProfile::from_value(1), UiProfile::Pocket);
}

#[test]
fn an_unknown_profile_value_is_classic() {
    for value in [2, 3, -1, i32::MAX, i32::MIN] {
        assert_eq!(UiProfile::from_value(value), UiProfile::Classic, "{value}");
    }
}

#[test]
fn the_screen_calls_resolve_from_their_five_entries_and_name_the_first_missing_one() {
    let host = FakeHost::new(&ScreenCalls::ENTRIES.map(|entry| (entry, 0x20)));
    assert!(ScreenCalls::resolve(&host).is_ok());

    let partial = FakeHost::new(&[(ScreenCalls::VISUAL_TREE, 0x50)]);
    let error = ScreenCalls::resolve(&partial).unwrap_err().to_string();
    assert!(error.contains(ScreenCalls::ROOT_CONTROL), "{error}");
}
