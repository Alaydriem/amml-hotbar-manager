use std::path::Path;

use alaydriem_hotbar_manager::HotbarManagerMod;
use alaydriem_hotbar_manager::services::{ButtonPainter, HotbarRuntime};
use amm_client::test_support::FakeTable;
use amml_sdk::{AmmlMod, SdkError};

use alaydriem_hotbar_manager::game::TopScreen;
use alaydriem_hotbar_manager::traits::GameHost;
use alaydriem_hotbar_manager::types::{ButtonCalls, ContainerCalls, PlayerCalls, RotationCalls, ScreenCalls};
use hotbar_manager_core::structs::Direction;

use crate::support::fake_host::FakeHost;

fn runtime() -> HotbarRuntime {
    HotbarRuntime::new(Path::new(FakeTable::CONFIG_DIR))
}

#[test]
fn the_mod_id_is_the_namespaced_id_and_amml_hcl_names_the_same_parts() {
    let document: hcl::Value = hcl::from_str(include_str!("../amml.hcl")).unwrap();
    let hcl::Value::Object(map) = document else { panic!("the document is not an object") };

    assert_eq!(HotbarManagerMod::id(), "@alaydriem/hotbar-manager");
    assert_eq!(HotbarManagerMod::id(), HotbarManagerMod::MODULE);
    assert_eq!(map.get("namespace").and_then(hcl::Value::as_str), Some(HotbarManagerMod::NAMESPACE));
    assert_eq!(map.get("name").and_then(hcl::Value::as_str), Some(HotbarManagerMod::NAME));
}

// every entry the mod resolves, each with a fake value
fn all_entries() -> Vec<(&'static str, u64)> {
    HotbarManagerMod::entries().into_iter().zip(0x1000..).collect()
}

const BUTTON_HOOKS: [&str; 2] = [HotbarManagerMod::BUTTON_EVENT, HotbarManagerMod::SCREEN_RENDER];
const ROTATION_HOOKS: [&str; 2] = [HotbarManagerMod::CONTAINER_OPEN, HotbarManagerMod::CONTAINER_CLOSE];

#[test]
fn enable_hooks_the_client_update_the_stack_response_the_two_button_hooks_and_the_two_container_hooks() {
    let host = FakeHost::new(&all_entries());

    HotbarManagerMod::enable_with(&host, &runtime()).unwrap();

    assert_eq!(
        host.hooked(),
        [
            HotbarManagerMod::UPDATE,
            HotbarManagerMod::STACK_RESPONSE,
            HotbarManagerMod::BUTTON_EVENT,
            HotbarManagerMod::SCREEN_RENDER,
            ROTATION_HOOKS[0],
            ROTATION_HOOKS[1],
        ]
    );
    assert!(host.warnings().is_empty());
}

#[test]
fn the_stack_response_hook_goes_on_the_handler_slot_of_the_network_handler_vtable() {
    let host = FakeHost::new(&all_entries());

    HotbarManagerMod::enable_with(&host, &runtime()).unwrap();

    let table = host.resolve(HotbarManagerMod::NETWORK_HANDLER_VTABLE).unwrap();
    let slot = host.resolve(HotbarManagerMod::STACK_RESPONSE).unwrap();
    assert_eq!(host.hooked_targets()[1], FakeHost::table_target(table, slot));
}

#[test]
fn enable_without_the_move_entries_keeps_the_client_update_and_turns_the_moves_off() {
    let host = FakeHost::new(&[(HotbarManagerMod::UPDATE, 0x1000)]);
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert_eq!(host.hooked(), [HotbarManagerMod::UPDATE]);
    assert!(host.warnings().iter().any(|line| line.contains(HotbarManagerMod::STACK_RESPONSE)));
    assert!(!runtime.moves_ready());
}

#[test]
fn enable_with_every_entry_makes_the_moves_ready() {
    let host = FakeHost::new(&all_entries());
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert!(runtime.moves_ready());
}

#[test]
fn amml_hcl_lists_every_entry_the_mod_resolves() {
    let document: hcl::Value = hcl::from_str(include_str!("../amml.hcl")).unwrap();
    let listed: Vec<&str> = document
        .as_object()
        .and_then(|map| map.get("entries"))
        .and_then(hcl::Value::as_array)
        .unwrap()
        .iter()
        .filter_map(hcl::Value::as_str)
        .collect();

    for entry in HotbarManagerMod::entries() {
        assert!(listed.contains(&entry), "{entry} is not in amml.hcl");
    }
}

#[test]
fn enable_fails_without_the_client_update_entry() {
    let host = FakeHost::new(&[]);

    let result = HotbarManagerMod::enable_with(&host, &runtime());

    assert!(matches!(result, Err(SdkError::MissingEntry(_))));
    assert!(host.hooked().is_empty());
}

fn every_entry_but(missing: &[&str]) -> Vec<(&'static str, u64)> {
    all_entries().into_iter().filter(|(entry, _)| !missing.contains(entry)).collect()
}

#[test]
fn a_missing_response_hook_means_no_moves_start() {
    let host = FakeHost::new(&every_entry_but(&[HotbarManagerMod::STACK_RESPONSE]));
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert_eq!(
        host.hooked(),
        [HotbarManagerMod::UPDATE, BUTTON_HOOKS[0], BUTTON_HOOKS[1], ROTATION_HOOKS[0], ROTATION_HOOKS[1]]
    );
    assert!(!runtime.moves_ready());
    assert!(runtime.rotation_blocked().is_some());
}

#[test]
fn the_moves_are_ready_when_every_entry_is_there() {
    let host = FakeHost::new(&all_entries());
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert!(runtime.moves_ready());
}

#[test]
fn an_optional_hook_that_does_not_go_in_turns_off_only_the_moves() {
    let host = FakeHost::new(&all_entries()).failing_hooks(&[HotbarManagerMod::STACK_RESPONSE]);
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert_eq!(
        host.hooked(),
        [HotbarManagerMod::UPDATE, BUTTON_HOOKS[0], BUTTON_HOOKS[1], ROTATION_HOOKS[0], ROTATION_HOOKS[1]]
    );
    assert!(runtime.buttons_ready());
    assert!(host.warnings().iter().any(|line| line.contains(HotbarManagerMod::STACK_RESPONSE)));
    assert!(!runtime.moves_ready());
}

#[test]
fn a_failed_required_hook_still_fails_enable() {
    let host = FakeHost::new(&all_entries()).failing_hooks(&[HotbarManagerMod::UPDATE]);

    assert!(HotbarManagerMod::enable_with(&host, &runtime()).is_err());
}

#[test]
fn without_the_inventory_offset_entries_the_moves_run_and_nothing_is_predicted() {
    for missing in [PlayerCalls::INVENTORY, PlayerCalls::ITEMS] {
        let host = FakeHost::new(&every_entry_but(&[missing]));
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        assert!(runtime.moves_ready());
        assert!(!runtime.with_moves(|engine| engine.predictor().bound()), "{missing}");
        assert!(host.warnings().iter().any(|line| line.contains(missing)), "{missing}");
    }
}

#[test]
fn with_every_entry_the_player_prediction_is_bound() {
    let host = FakeHost::new(&all_entries());
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert!(runtime.with_moves(|engine| engine.predictor().bound()));
}

#[test]
fn with_every_entry_the_buttons_are_bound() {
    let host = FakeHost::new(&all_entries());
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert!(runtime.buttons_ready());
}

#[test]
fn without_the_input_mode_entry_the_buttons_are_bound_with_one_warning() {
    let host = FakeHost::new(&every_entry_but(&[ButtonCalls::INPUT_MODE]));
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert!(runtime.buttons_ready());
    let warnings = host.warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains(ButtonCalls::INPUT_MODE), "{warnings:?}");
}

#[test]
fn a_missing_button_entry_turns_off_only_the_buttons() {
    for missing in [ButtonCalls::DRAW_IMAGE, ButtonCalls::POINTER_BUTTON_IDS, ScreenCalls::CONTROL_NAME] {
        let host = FakeHost::new(&every_entry_but(&[missing]));
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        assert!(!runtime.buttons_ready(), "{missing}");
        assert!(runtime.moves_ready(), "{missing}");
        assert!(host.warnings().iter().any(|line| line.contains(missing)), "{missing}");
    }
}

#[test]
fn the_buttons_are_off_unless_both_button_hooks_go_in() {
    for failing in BUTTON_HOOKS {
        let host = FakeHost::new(&all_entries()).failing_hooks(&[failing]);
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        assert!(!runtime.buttons_ready(), "{failing}");
        assert!(host.hooked().contains(&HotbarManagerMod::UPDATE), "{failing}");
    }
    for missing in BUTTON_HOOKS {
        let host = FakeHost::new(&every_entry_but(&[missing]));
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        assert!(!runtime.buttons_ready(), "{missing}");
    }
}

#[test]
fn the_textures_folder_holds_every_glyph_the_painter_loads() {
    for glyph in ButtonPainter::GLYPHS {
        let file = format!("{}/textures/{}.png", env!("CARGO_MANIFEST_DIR"), ButtonPainter::glyph_file(glyph));
        assert!(Path::new(&file).is_file(), "{file} is missing");
    }
}

#[test]
fn without_the_image_queue_entry_the_buttons_are_bound_with_one_warning() {
    let host = FakeHost::new(&every_entry_but(&[ButtonCalls::QUEUED_IMAGES]));
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert!(runtime.buttons_ready());
    let warnings = host.warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains(ButtonCalls::QUEUED_IMAGES), "{warnings:?}");
}

#[test]
fn the_optional_button_entries_resolve_to_none_when_missing_and_to_their_value_when_present() {
    let calls = ButtonCalls::resolve(&FakeHost::new(&all_entries())).unwrap();
    let host = FakeHost::new(&all_entries());
    assert_eq!(calls.queued_images, host.resolve(ButtonCalls::QUEUED_IMAGES).ok());
    assert_eq!(calls.input_mode, host.resolve(ButtonCalls::INPUT_MODE).ok());

    let partial = every_entry_but(&[ButtonCalls::QUEUED_IMAGES, ButtonCalls::INPUT_MODE]);
    let calls = ButtonCalls::resolve(&FakeHost::new(&partial)).unwrap();
    assert_eq!((calls.queued_images, calls.input_mode), (None, None));
}

#[test]
fn the_container_hooks_go_on_the_handler_slots_of_the_network_handler_vtable() {
    let host = FakeHost::new(&all_entries());

    HotbarManagerMod::enable_with(&host, &runtime()).unwrap();

    let table = host.resolve(HotbarManagerMod::NETWORK_HANDLER_VTABLE).unwrap();
    for (offset, entry) in ROTATION_HOOKS.into_iter().enumerate() {
        let slot = host.resolve(entry).unwrap();
        assert_eq!(host.hooked_targets()[4 + offset], FakeHost::table_target(table, slot), "{entry}");
    }
}

#[test]
fn without_the_inventory_offset_entries_the_rotation_is_off_and_names_them() {
    for missing in [PlayerCalls::INVENTORY, PlayerCalls::ITEMS] {
        let host = FakeHost::new(&every_entry_but(&[missing]));
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        let reason = runtime.rotation_blocked().unwrap();
        assert!(reason.contains(missing), "{reason}");
    }
}

#[test]
fn without_the_runtime_id_or_the_top_screen_entry_the_rotation_is_off_and_names_it() {
    for missing in [RotationCalls::GET_RUNTIME_ID, TopScreen::ENTRY] {
        let host = FakeHost::new(&every_entry_but(&[missing]));
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        let reason = runtime.rotation_blocked().unwrap();
        assert!(reason.contains(missing), "{reason}");
    }
}

#[test]
fn a_container_hook_that_does_not_go_in_turns_off_only_the_rotation() {
    for entry in ROTATION_HOOKS {
        let host = FakeHost::new(&all_entries()).failing_hooks(&[entry]);
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        assert!(runtime.moves_ready());
        assert!(runtime.buttons_ready());
        assert!(runtime.rotation_blocked().unwrap().contains("is not hooked"));
        assert!(host.warnings().iter().any(|line| line.contains(entry)));
    }
}

#[test]
fn disable_turns_the_rotation_off() {
    let runtime = runtime();
    HotbarManagerMod::enable_with(&FakeHost::new(&all_entries()), &runtime).unwrap();

    runtime.bind_rotation(None, None);
    runtime.stop_rotation();

    assert!(!runtime.rotation_active());
}

#[test]
fn a_rotation_key_while_the_rotation_is_off_starts_nothing() {
    let host = FakeHost::new(&every_entry_but(&[PlayerCalls::ITEMS]));
    let runtime = runtime();
    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    runtime.on_rotate(Direction::Down, 1.0);
    runtime.on_rotate(Direction::Up, 1.0);

    assert!(!runtime.rotation_active());
}

#[test]
fn the_mod_lists_every_container_entry() {
    let document: hcl::Value = hcl::from_str(include_str!("../amml.hcl")).unwrap();
    let listed: Vec<&str> = document
        .as_object()
        .and_then(|map| map.get("entries"))
        .and_then(hcl::Value::as_array)
        .unwrap()
        .iter()
        .filter_map(hcl::Value::as_str)
        .collect();

    for entry in ContainerCalls::ENTRIES {
        assert!(HotbarManagerMod::entries().contains(&entry), "{entry} is not in entries()");
        assert!(listed.contains(&entry), "{entry} is not in amml.hcl");
    }
}

#[test]
fn with_every_entry_the_screen_actions_are_bound() {
    let host = FakeHost::new(&all_entries());
    let runtime = runtime();

    HotbarManagerMod::enable_with(&host, &runtime).unwrap();

    assert!(runtime.screen_actions_ready());
}

#[test]
fn a_missing_container_entry_turns_off_the_screen_actions_with_one_warning() {
    for entry in ContainerCalls::ENTRIES {
        let host = FakeHost::new(&every_entry_but(&[entry]));
        let runtime = runtime();

        HotbarManagerMod::enable_with(&host, &runtime).unwrap();

        assert!(!runtime.screen_actions_ready(), "{entry}");
        let warnings = host.warnings();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0], format!("{entry} did not resolve; the screen actions (sort, dump, drop) are off"));
    }
}

#[test]
fn the_selected_hotbar_slot_is_no_longer_read() {
    // the filtered buttons match the stack held on the cursor (2026-11-09); nothing reads PlayerInventory.mSelected
    assert!(!HotbarManagerMod::entries().contains(&"PlayerInventory.mSelected"));
    assert!(!include_str!("../amml.hcl").contains("PlayerInventory.mSelected"));
}
