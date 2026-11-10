use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use alaydriem_hotbar_manager::services::{ButtonUi, ClickRouter, HotbarRuntime, ViewLayout};
use alaydriem_hotbar_manager::types::{ClickRoute, InputMode, UiProfile};
use amm_client::test_support::FakeTable;
use hotbar_manager_core::structs::{ButtonAction, PanelRect, ScreenKind};

const VIEW: usize = 0x1000;
// the second, taller render of the same screen
const TALL_VIEW: usize = 0x2000;
const PANEL: PanelRect = PanelRect { x: 272.0, y: 542.5, w: 176.0, h: 166.0 };
const TALL_PANEL: PanelRect = PanelRect { x: 272.0, y: 1168.0, w: 176.0, h: 166.0 };
const MOUSE: Option<InputMode> = Some(InputMode::Mouse);

fn layout(view: usize, panel: PanelRect) -> ViewLayout {
    ViewLayout { view, panel, specs: ClickRouter::buttons(ScreenKind::SmallChest, UiProfile::Classic) }
}

// the centre of Sort on `panel`
fn sort(panel: PanelRect) -> (f32, f32) {
    (panel.x + 147.5, panel.y + 12.5)
}

#[test]
fn a_click_on_a_drawn_button_is_routed_against_the_view_layout() {
    let mut ui = ButtonUi::new();
    ui.remember(layout(VIEW, PANEL));

    assert_eq!(ui.route(VIEW, 0, true, sort(PANEL), MOUSE), ClickRoute::Swallow);
    assert_eq!(ui.route(VIEW, 0, false, sort(PANEL), MOUSE), ClickRoute::Fire(ButtonAction::Sort));
    assert_eq!(ButtonUi::fired(ClickRoute::Fire(ButtonAction::Sort)), Some(ButtonAction::Sort));
    assert_eq!(ButtonUi::fired(ClickRoute::Swallow), None);
}

#[test]
fn a_view_with_no_drawn_layout_takes_no_press() {
    let mut ui = ButtonUi::new();
    ui.remember(layout(TALL_VIEW, TALL_PANEL));

    assert_eq!(ui.route(VIEW, 0, true, sort(PANEL), MOUSE), ClickRoute::Pass);
}

#[test]
fn each_view_is_tested_against_its_own_panel() {
    let mut ui = ButtonUi::new();
    ui.remember(layout(VIEW, PANEL));
    ui.remember(layout(TALL_VIEW, TALL_PANEL));

    // the real view's Sort point is a slot area on the tall view's panel, and the other way round
    assert_eq!(ui.route(TALL_VIEW, 0, true, sort(PANEL), MOUSE), ClickRoute::Pass);
    assert_eq!(ui.route(VIEW, 0, true, sort(PANEL), MOUSE), ClickRoute::Swallow);
}

#[test]
fn the_second_render_never_clears_the_held_press() {
    let mut ui = ButtonUi::new();
    ui.remember(layout(VIEW, PANEL));

    assert_eq!(ui.route(VIEW, 0, true, sort(PANEL), MOUSE), ClickRoute::Swallow);
    // frames go by: both views draw, the real one first
    for _ in 0..3 {
        ui.remember(layout(VIEW, PANEL));
        ui.remember(layout(TALL_VIEW, TALL_PANEL));
    }

    assert_eq!(ui.route(VIEW, 0, false, sort(PANEL), MOUSE), ClickRoute::Fire(ButtonAction::Sort));
}

// the game hands each pointer event to every view of the screen stack: the screen's view first, then views that draw
// no buttons (on 1.26.63.01 two of them, live on every screen; docs/findings/debug-input/report.md)
#[test]
fn a_click_that_also_reaches_views_with_no_buttons_still_fires() {
    const HUD_VIEWS: [usize; 2] = [0x3000, 0x4000];
    let mut ui = ButtonUi::new();
    ui.remember(layout(VIEW, PANEL));

    assert_eq!(ui.route(VIEW, 0, true, sort(PANEL), MOUSE), ClickRoute::Swallow);
    for view in HUD_VIEWS {
        assert_eq!(ui.route(view, 0, true, sort(PANEL), MOUSE), ClickRoute::Pass);
    }
    assert_eq!(ui.route(VIEW, 0, false, sort(PANEL), MOUSE), ClickRoute::Fire(ButtonAction::Sort));
    for view in HUD_VIEWS {
        assert_eq!(ui.route(view, 0, false, sort(PANEL), MOUSE), ClickRoute::Pass);
    }
}

#[test]
fn the_held_view_drawing_no_buttons_clears_the_press_and_another_view_does_not() {
    let mut ui = ButtonUi::new();
    ui.remember(layout(VIEW, PANEL));
    ui.route(VIEW, 0, true, sort(PANEL), MOUSE);

    // a HUD view draws nothing every frame
    ui.forget(0x3000);
    ui.remember(layout(VIEW, PANEL));
    assert_eq!(ui.route(VIEW, 0, false, (0.0, 0.0), MOUSE), ClickRoute::Swallow);

    ui.route(VIEW, 0, true, sort(PANEL), MOUSE);
    // the screen closes: the view shows no buttons
    ui.forget(VIEW);
    assert!(ui.layout(VIEW).is_none());
    assert_eq!(ui.route(VIEW, 0, false, sort(PANEL), MOUSE), ClickRoute::Pass);
}

#[test]
fn at_most_four_views_are_kept_newest_first() {
    let mut ui = ButtonUi::new();
    for view in 1..=6 {
        ui.remember(layout(view, PANEL));
    }

    for view in 3..=6 {
        assert!(ui.layout(view).is_some(), "{view}");
    }
    assert!(ui.layout(1).is_none());
    assert!(ui.layout(2).is_none());
}

#[test]
fn binding_clears_the_layouts_and_the_held_presses() {
    let mut ui = ButtonUi::new();
    ui.remember(layout(VIEW, PANEL));
    ui.route(VIEW, 0, true, sort(PANEL), MOUSE);

    ui.bind(None);

    assert!(ui.layout(VIEW).is_none());
    assert_eq!(ui.route(VIEW, 0, false, sort(PANEL), MOUSE), ClickRoute::Pass);
}

#[test]
fn an_unbound_runtime_passes_every_button_event_to_the_game() {
    let runtime = HotbarRuntime::new(Path::new(FakeTable::CONFIG_DIR));

    assert!(!runtime.buttons_ready());
    assert!(!runtime.on_button_event(VIEW, 1, true));
    assert!(!runtime.on_button_event(VIEW, 1, false));
}

#[test]
fn the_textures_dir_is_given_with_forward_slashes() {
    let runtime = HotbarRuntime::new(Path::new(FakeTable::CONFIG_DIR));
    assert_eq!(runtime.textures_dir(), None);

    runtime.set_textures_dir(r"C:\mods\amml\@alaydriem\hotbar-manager\textures".into());

    assert_eq!(runtime.textures_dir().as_deref(), Some("C:/mods/amml/@alaydriem/hotbar-manager/textures"));
}

// runs `call` on another thread while this thread holds the buttons' lock; the result, or Err when the call is still
// blocked after 2 seconds
fn while_locked<R: Send>(
    runtime: &HotbarRuntime,
    call: impl FnOnce(&HotbarRuntime) -> R + Send,
) -> Result<R, RecvTimeoutError> {
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        runtime.with_buttons(|_| {
            scope.spawn(move || {
                let _ = sender.send(call(runtime));
            });
            receiver.recv_timeout(Duration::from_secs(2))
        })
    })
}

#[test]
fn a_click_while_the_buttons_lock_is_held_passes_to_the_game_without_blocking() {
    let runtime = HotbarRuntime::new(Path::new(FakeTable::CONFIG_DIR));

    assert_eq!(while_locked(&runtime, |runtime| runtime.on_button_event(VIEW, 1, true)), Ok(false));
    assert_eq!(while_locked(&runtime, |runtime| runtime.on_button_event(VIEW, 1, false)), Ok(false));
}

#[test]
fn a_render_while_the_buttons_lock_is_held_skips_the_buttons_without_blocking() {
    let runtime = HotbarRuntime::new(Path::new(FakeTable::CONFIG_DIR));

    assert_eq!(while_locked(&runtime, |runtime| runtime.on_screen_render(VIEW, 0x3000)), Ok(()));
}
