use alaydriem_hotbar_manager::services::{ScreenGuard, Verdict};
use hotbar_manager_core::structs::ScreenKind;

fn bound_on(name: &str) -> ScreenGuard {
    let mut guard = ScreenGuard::new();
    guard.top_changed(name);
    guard.bind();
    guard
}

#[test]
fn a_frame_hitch_changes_nothing_because_time_is_not_part_of_the_identity() {
    let mut guard = bound_on("small_chest_screen");
    guard.frame(0xA);

    // many ticks and frames of the same screen, however far apart: nothing cancels
    for _ in 0..1000 {
        assert_eq!(guard.top_changed("small_chest_screen"), Verdict::Keep);
        assert_eq!(guard.frame(0xA), Verdict::Keep);
    }
}

#[test]
fn a_close_cancels_on_the_next_tick() {
    let mut guard = bound_on("small_chest_screen");
    guard.frame(0xA);

    assert_eq!(guard.top_changed("hud_screen"), Verdict::Cancel);
}

#[test]
fn a_switch_to_another_screen_name_cancels() {
    let mut guard = bound_on("small_chest_screen");

    assert_eq!(guard.top_changed("large_chest_screen"), Verdict::Cancel);
}

#[test]
fn a_switch_to_another_container_of_the_same_kind_cancels_at_its_first_frame() {
    let mut guard = bound_on("small_chest_screen");
    guard.frame(0xA);

    // no HUD tick between: the name is the same, the controller is not
    assert_eq!(guard.top_changed("small_chest_screen"), Verdict::Keep);
    assert_eq!(guard.frame(0xB), Verdict::Cancel);
}

#[test]
fn once_cancelled_the_guard_is_unbound_and_cancels_nothing_more() {
    let mut guard = bound_on("small_chest_screen");
    guard.frame(0xA);
    guard.frame(0xB);

    assert_eq!(guard.frame(0xC), Verdict::Keep);
    assert_eq!(guard.top_changed("hud_screen"), Verdict::Keep);
}

#[test]
fn a_toast_changes_nothing() {
    let mut guard = bound_on("small_chest_screen");
    guard.frame(0xA);

    assert_eq!(guard.top_changed("toast_screen"), Verdict::Keep);
    assert_eq!(guard.top_changed("small_chest_screen"), Verdict::Keep);
    assert_eq!(guard.frame(0xA), Verdict::Keep);
}

#[test]
fn an_unbound_guard_never_cancels() {
    let mut guard = ScreenGuard::new();

    assert_eq!(guard.top_changed("small_chest_screen"), Verdict::Keep);
    assert_eq!(guard.frame(0xA), Verdict::Keep);
    assert_eq!(guard.top_changed("hud_screen"), Verdict::Keep);
}

#[test]
fn a_release_unbinds() {
    let mut guard = bound_on("small_chest_screen");
    guard.release();

    assert_eq!(guard.top_changed("hud_screen"), Verdict::Keep);
}

#[test]
fn the_top_screen_gives_the_menu_state_and_the_kind() {
    let mut guard = ScreenGuard::new();
    assert!(!guard.top_known());
    assert!(!guard.menu_open());

    guard.top_changed("hud_screen");
    assert!(guard.top_known());
    assert!(!guard.menu_open());

    guard.top_changed("hopper_screen");
    assert!(guard.menu_open());
    assert_eq!(guard.top_kind(), ScreenKind::Hopper);

    guard.top_changed("some_unknown_screen");
    assert!(guard.menu_open());
    assert_eq!(guard.top_kind(), ScreenKind::Other);
}

#[test]
fn the_hud_is_the_top_screen_only_when_the_name_says_so() {
    let mut guard = ScreenGuard::new();
    assert!(!guard.hud());

    guard.top_changed("hud_screen");
    assert!(guard.hud());

    guard.top_changed("toast_screen");
    assert!(guard.hud());

    guard.top_changed("inventory_screen");
    assert!(!guard.hud());

    guard.top_changed("start_screen");
    assert!(!guard.hud());
}
