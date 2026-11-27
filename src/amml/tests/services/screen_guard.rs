use alaydriem_hotbar_manager::services::{ScreenGuard, Verdict};
use amml_api::{Screen, ScreenId};

const CHEST: ScreenId = ScreenId(7);
const OTHER_CHEST: ScreenId = ScreenId(9);

// a guard with a container screen on top and an action bound to it
fn bound_on_container() -> ScreenGuard {
    let mut guard = ScreenGuard::new();
    guard.top_changed(Screen::Container);
    guard.bind();
    guard
}

#[test]
fn a_frame_hitch_changes_nothing_because_time_is_not_part_of_the_identity() {
    let mut guard = bound_on_container();

    assert_eq!(guard.frame(CHEST), Verdict::Keep);
    assert_eq!(guard.top_changed(Screen::Container), Verdict::Keep);
    assert_eq!(guard.frame(CHEST), Verdict::Keep);
}

#[test]
fn a_close_cancels_on_the_next_tick() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);

    assert_eq!(guard.top_changed(Screen::Hud), Verdict::Cancel);
}

#[test]
fn a_screen_changed_to_the_hud_cancels() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);

    assert_eq!(guard.screen_changed(ScreenId(3), Screen::Hud), Verdict::Cancel);
}

#[test]
fn a_switch_to_another_container_cancels_at_its_screen_changed() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);

    assert_eq!(guard.screen_changed(OTHER_CHEST, Screen::Container), Verdict::Cancel);
}

#[test]
fn a_switch_to_another_container_that_never_shows_the_hud_cancels_at_its_first_frame() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);

    assert_eq!(guard.frame(OTHER_CHEST), Verdict::Cancel);
}

#[test]
fn a_screen_changed_with_no_screen_cancels_an_action_that_has_no_frame_yet() {
    let mut guard = bound_on_container();

    assert_eq!(guard.screen_changed(ScreenId::NONE, Screen::Container), Verdict::Cancel);
}

#[test]
fn a_screen_changed_to_the_same_screen_keeps_the_action() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);

    assert_eq!(guard.screen_changed(CHEST, Screen::Container), Verdict::Keep);
}

#[test]
fn once_cancelled_the_guard_is_unbound_and_cancels_nothing_more() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);
    assert_eq!(guard.top_changed(Screen::Hud), Verdict::Cancel);

    assert_eq!(guard.top_changed(Screen::Container), Verdict::Keep);
    assert_eq!(guard.frame(OTHER_CHEST), Verdict::Keep);
}

#[test]
fn a_toast_changes_nothing() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);

    assert_eq!(guard.top_changed(Screen::Toast), Verdict::Keep);
    assert_eq!(guard.screen_changed(ScreenId(5), Screen::Toast), Verdict::Keep);
    assert!(guard.container_open());
    assert_eq!(guard.top_changed(Screen::Container), Verdict::Keep);
}

#[test]
fn an_unbound_guard_never_cancels() {
    let mut guard = ScreenGuard::new();

    assert_eq!(guard.top_changed(Screen::Container), Verdict::Keep);
    assert_eq!(guard.top_changed(Screen::Hud), Verdict::Keep);
    assert_eq!(guard.frame(CHEST), Verdict::Keep);
    assert_eq!(guard.frame(OTHER_CHEST), Verdict::Keep);
}

#[test]
fn a_release_unbinds() {
    let mut guard = bound_on_container();
    guard.frame(CHEST);
    guard.release();

    assert_eq!(guard.top_changed(Screen::Hud), Verdict::Keep);
    assert_eq!(guard.frame(OTHER_CHEST), Verdict::Keep);
}

#[test]
fn the_top_screen_gives_the_container_and_hud_state() {
    let mut guard = ScreenGuard::new();
    assert!(!guard.top_known());
    assert!(!guard.container_open());
    assert!(!guard.hud());

    guard.top_changed(Screen::Container);
    assert!(guard.top_known() && guard.container_open() && !guard.hud());

    guard.top_changed(Screen::Hud);
    assert!(guard.top_known() && !guard.container_open() && guard.hud());

    guard.top_changed(Screen::Pause);
    assert!(!guard.container_open() && !guard.hud());
}
