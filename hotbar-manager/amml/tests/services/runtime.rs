use std::path::Path;

use alaydriem_hotbar_manager::HotbarManagerMod;
use alaydriem_hotbar_manager::game::ScreenActions;
use alaydriem_hotbar_manager::services::{HotbarDescriptor, HotbarRuntime, ScreenWatch};
use alaydriem_hotbar_manager::types::ContainerCalls;
use amm_client::test_support::FakeTable;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use hotbar_manager_core::structs::{ButtonAction, Direction, ScreenKind, ScreenStep, Side, SlotAddr};

use crate::support::fake_actions::FakeActions;
use crate::support::fake_host::FakeHost;
use crate::support::fake_screen::{FakeScreen, model, stack};

fn runtime() -> HotbarRuntime {
    HotbarRuntime::new(Path::new(FakeTable::CONFIG_DIR))
}

fn all_entries() -> Vec<(&'static str, u64)> {
    HotbarManagerMod::entries().into_iter().zip(0x1000..).collect()
}

// a runtime with the moves on, as enable makes it
fn enabled() -> HotbarRuntime {
    let runtime = runtime();
    HotbarManagerMod::enable_with(&FakeHost::new(&all_entries()), &runtime).unwrap();
    runtime
}

fn press(action: u16) -> ModEvent {
    ModEvent::Keybind { action, phase: KeybindPhase::Pressed }
}

// a small chest with dirt in slot 0 and apple in slot 2: the sort needs a few steps; the player holds stone in main 12
fn chest() -> FakeScreen {
    let mut shown = model(27);
    shown.container[0] = stack("dirt", 0, 5, 1);
    shown.container[2] = stack("apple", 0, 3, 2);
    shown.player[12] = stack("stone", 0, 4, 9);
    FakeScreen::new(ScreenKind::SmallChest, shown)
}

// one frame of the screen drawn by `controller`, with the container handlers `actions`
fn frame(runtime: &HotbarRuntime, screen: &FakeScreen, controller: usize, actions: &mut FakeActions) {
    runtime.run_frame(screen, Some(controller), Some(actions as &mut dyn ScreenActions));
}

// the chest is open (controller 0xA) and a sort runs: its first step ran in the first frame
fn sorting(runtime: &HotbarRuntime, screen: &FakeScreen, actions: &mut FakeActions) {
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);
    frame(runtime, screen, 0xA, actions);
    assert!(runtime.sort_running());
    assert_eq!(actions.steps.len(), 1);
}

#[test]
fn the_sort_keybind_is_the_third_declared_in_the_descriptor() {
    let declared: Vec<&str> = HotbarDescriptor::TEXT
        .lines()
        .filter_map(|line| line.trim().strip_prefix("keybind \""))
        .filter_map(|rest| rest.split('"').next())
        .collect();

    assert_eq!(declared.iter().position(|name| *name == "sort"), Some(usize::from(HotbarRuntime::SORT_KEYBIND)));
}

#[test]
fn the_sort_keybind_with_a_screen_open_waits_for_the_next_frame() {
    let runtime = enabled();
    runtime.note_frame(0x10, 1.0);

    runtime.on_event(&press(HotbarRuntime::SORT_KEYBIND), 1.0);

    assert_eq!(runtime.pending_action(), Some(ButtonAction::Sort));
}

#[test]
fn the_sort_key_with_no_screen_open_does_nothing() {
    let runtime = enabled();

    runtime.on_event(&press(HotbarRuntime::SORT_KEYBIND), 1.0);

    assert_eq!(runtime.pending_action(), None);
}

#[test]
fn the_rotation_keybinds_are_the_first_two_declared_in_the_descriptor() {
    let declared: Vec<&str> = HotbarDescriptor::TEXT
        .lines()
        .filter_map(|line| line.trim().strip_prefix("keybind \""))
        .filter_map(|rest| rest.split('"').next())
        .collect();

    assert_eq!(
        declared.iter().position(|name| *name == "rotate_up"),
        Some(usize::from(HotbarRuntime::ROTATE_UP_KEYBIND))
    );
    assert_eq!(
        declared.iter().position(|name| *name == "rotate_down"),
        Some(usize::from(HotbarRuntime::ROTATE_DOWN_KEYBIND))
    );
}

#[test]
fn a_release_a_repeat_and_the_rotation_keybinds_do_not_sort() {
    let runtime = enabled();
    runtime.note_frame(0x10, 1.0);
    let events = [
        ModEvent::Keybind { action: HotbarRuntime::SORT_KEYBIND, phase: KeybindPhase::Released },
        ModEvent::Keybind { action: HotbarRuntime::SORT_KEYBIND, phase: KeybindPhase::Repeat },
        press(0),
        press(1),
    ];

    for event in &events {
        runtime.on_event(event, 1.0);
    }

    assert_eq!(runtime.pending_action(), None);
}

#[test]
fn a_button_action_while_the_moves_are_off_does_nothing() {
    let runtime = runtime();
    runtime.note_frame(0x10, 1.0);

    runtime.on_action(ButtonAction::DropAll, 1.0);

    assert_eq!(runtime.pending_action(), None);
}

#[test]
fn a_second_action_while_one_waits_is_ignored() {
    let runtime = enabled();
    runtime.note_frame(0x10, 1.0);

    runtime.on_action(ButtonAction::DumpFromInventory, 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);

    assert_eq!(runtime.pending_action(), Some(ButtonAction::DumpFromInventory));
}

#[test]
fn a_screen_that_stops_drawing_drops_the_waiting_action() {
    let runtime = enabled();
    runtime.note_frame(0x10, 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);

    runtime.sweep(1.0 + ScreenWatch::OPEN_SECONDS + 0.01);

    assert_eq!(runtime.pending_action(), None);
}

#[test]
fn a_screen_that_keeps_drawing_keeps_the_waiting_action() {
    let runtime = enabled();
    runtime.note_frame(0x10, 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);

    runtime.sweep(1.0 + ScreenWatch::OPEN_SECONDS);

    assert_eq!(runtime.pending_action(), Some(ButtonAction::Sort));
}

#[test]
fn enable_with_every_entry_warns_of_nothing_new() {
    let host = FakeHost::new(&all_entries());

    HotbarManagerMod::enable_with(&host, &runtime()).unwrap();

    assert!(host.warnings().is_empty(), "{:?}", host.warnings());
}

#[test]
fn a_missing_screen_read_entry_is_named_in_a_warning() {
    let entries: Vec<_> = all_entries()
        .into_iter()
        .filter(|(entry, _)| *entry != "ContainerScreenController.getVisualItemStackImpl")
        .collect();
    let host = FakeHost::new(&entries);

    HotbarManagerMod::enable_with(&host, &runtime()).unwrap();

    assert!(host.warnings().iter().any(|line| line.contains("ContainerScreenController.getVisualItemStackImpl")));
}

fn enabled_without(missing: &[&str]) -> HotbarRuntime {
    let entries: Vec<_> = all_entries().into_iter().filter(|(entry, _)| !missing.contains(entry)).collect();
    let runtime = runtime();
    HotbarManagerMod::enable_with(&FakeHost::new(&entries), &runtime).unwrap();
    runtime
}

#[test]
fn the_sort_keybind_works_in_pocket_where_no_buttons_are_drawn() {
    // Pocket draws no buttons, so no frame ever marks a view; the top screen name says a container is open
    let runtime = enabled();
    runtime.on_top_screen("small_chest_screen", 1.0);

    runtime.on_event(&press(HotbarRuntime::SORT_KEYBIND), 1.0);

    assert_eq!(runtime.pending_action(), Some(ButtonAction::Sort));
}

#[test]
fn the_sort_keybind_works_when_the_buttons_are_unbound() {
    let runtime = enabled_without(&[HotbarManagerMod::BUTTON_EVENT]);
    assert!(!runtime.buttons_ready());
    runtime.on_top_screen("hopper_screen", 1.0);

    runtime.on_event(&press(HotbarRuntime::SORT_KEYBIND), 1.0);

    assert_eq!(runtime.pending_action(), Some(ButtonAction::Sort));
}

#[test]
fn the_sort_keybind_does_nothing_on_the_hud_or_the_player_inventory() {
    let runtime = enabled();

    for top in ["hud_screen", "inventory_screen", "inventory_screen_pocket"] {
        runtime.on_top_screen(top, 1.0);
        runtime.on_event(&press(HotbarRuntime::SORT_KEYBIND), 1.0);

        assert_eq!(runtime.pending_action(), None, "{top}");
    }
}

#[test]
fn a_close_drops_a_queued_action() {
    let runtime = enabled();
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);
    assert_eq!(runtime.pending_action(), Some(ButtonAction::Sort));

    runtime.on_top_screen("hud_screen", 1.01);

    assert_eq!(runtime.pending_action(), None);
}

#[test]
fn a_queued_action_does_not_run_on_another_container() {
    let runtime = enabled();
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::DumpFromInventory, 1.0);
    assert!(runtime.accept_frame(0xA));
    // the frame of chest B comes before the action started on chest A's frame was released
    assert!(!runtime.accept_frame(0xB));

    assert_eq!(runtime.pending_action(), None);
}

#[test]
fn a_dump_runs_in_its_first_frame_and_releases_the_screen() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::recording();
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::DumpFromContainer, 1.0);

    frame(&runtime, &screen, 0xA, &mut actions);

    let chest_slot = |slot| SlotAddr { side: Side::Container, slot };
    assert_eq!(actions.steps, [ScreenStep::AutoPlace(chest_slot(0)), ScreenStep::AutoPlace(chest_slot(2))]);
    assert_eq!(runtime.pending_action(), None);
    assert!(!runtime.sort_running());
    // released: a frame of another controller cancels nothing, and the next action queues
    assert!(runtime.accept_frame(0xB));
    runtime.on_action(ButtonAction::Sort, 1.1);
    assert_eq!(runtime.pending_action(), Some(ButtonAction::Sort));
}

#[test]
fn sort_runs_one_step_per_frame_until_done() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    sorting(&runtime, &screen, &mut actions);

    let mut frames = 1;
    while runtime.sort_running() {
        let before = actions.steps.len();
        runtime.next_tick();
        frame(&runtime, &screen, 0xA, &mut actions);
        frames += 1;
        assert!(actions.steps.len() <= before + 1, "one step per frame");
        assert!(frames < 100);
    }

    let ids: Vec<_> = screen.model.borrow().as_ref().unwrap().container[..3]
        .iter()
        .map(|slot| slot.as_ref().map(|item| item.id.clone()))
        .collect();
    assert_eq!(ids, [Some("minecraft:apple".to_string()), Some("minecraft:dirt".to_string()), None]);
    let steps = actions.steps.len();
    runtime.next_tick();
    frame(&runtime, &screen, 0xA, &mut actions);
    assert_eq!(actions.steps.len(), steps, "nothing runs after Done");
    // the guard is released: another controller cancels nothing, and a new action queues
    assert!(runtime.accept_frame(0xB));
    runtime.on_action(ButtonAction::Sort, 2.0);
    assert_eq!(runtime.pending_action(), Some(ButtonAction::Sort));
}

#[test]
fn a_screen_change_drops_the_sort() {
    // a close (the top screen changes)
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    sorting(&runtime, &screen, &mut actions);

    runtime.on_top_screen("hud_screen", 1.01);
    frame(&runtime, &screen, 0xA, &mut actions);

    assert!(!runtime.sort_running());
    assert_eq!(actions.steps.len(), 1, "no step after the close");

    // a switch to another container of the same kind (another controller, the same name)
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    sorting(&runtime, &screen, &mut actions);
    runtime.on_top_screen("small_chest_screen", 1.02);

    frame(&runtime, &screen, 0xB, &mut actions);
    frame(&runtime, &screen, 0xB, &mut actions);

    assert!(!runtime.sort_running());
    assert_eq!(actions.steps.len(), 1, "no step on the other controller");
}

#[test]
fn a_screen_that_stops_drawing_drops_the_sort() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    runtime.note_frame(0x10, 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);
    frame(&runtime, &screen, 0xA, &mut actions);
    assert!(runtime.sort_running());

    runtime.sweep(1.0 + ScreenWatch::OPEN_SECONDS + 0.01);

    assert!(!runtime.sort_running());
}

#[test]
fn a_frame_hitch_or_a_toast_does_not_drop_the_sort() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    sorting(&runtime, &screen, &mut actions);

    // five seconds with no frame; the top screen is still the chest; then a toast
    runtime.on_top_screen("small_chest_screen", 6.0);
    runtime.on_top_screen("toast_screen", 6.1);

    assert!(runtime.sort_running());
    assert!(runtime.accept_frame(0xA));
}

#[test]
fn an_action_while_a_sort_runs_is_ignored() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    sorting(&runtime, &screen, &mut actions);

    runtime.on_action(ButtonAction::DumpFromContainer, 1.1);

    assert_eq!(runtime.pending_action(), None);
    assert!(runtime.sort_running());
}

#[test]
fn a_rotation_key_while_a_sort_runs_is_ignored() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    sorting(&runtime, &screen, &mut actions);

    runtime.on_rotate(Direction::Up, 1.1);
    runtime.on_event(&press(HotbarRuntime::ROTATE_DOWN_KEYBIND), 1.1);

    assert!(!runtime.rotation_active());
    assert!(runtime.sort_running());
    // the key stopped at the sort check: it did not reach the rotation's own checks (which warn once, here because the
    // test runtime has no slot reader)
    assert!(!runtime.rotation_off_warned());
    runtime.on_top_screen("hud_screen", 1.2);
    runtime.on_rotate(Direction::Up, 1.2);
    assert!(runtime.rotation_off_warned(), "with no sort, the key reaches the rotation checks");
}

#[test]
fn screen_actions_off_ignore_every_button() {
    let runtime = enabled_without(&[ContainerCalls::ENTRIES[0]]);
    assert!(!runtime.screen_actions_ready());
    runtime.on_top_screen("small_chest_screen", 1.0);
    let actions = [
        ButtonAction::Sort,
        ButtonAction::DumpFromInventory,
        ButtonAction::DumpFromContainer,
        ButtonAction::DumpFromInventoryFiltered,
        ButtonAction::DumpFromContainerFiltered,
        ButtonAction::DropAll,
        ButtonAction::DropFiltered,
    ];

    for action in actions {
        runtime.on_action(action, 1.0);
        assert_eq!(runtime.pending_action(), None, "{action:?}");
    }
    runtime.on_event(&press(HotbarRuntime::SORT_KEYBIND), 1.0);
    assert_eq!(runtime.pending_action(), None);
    // the rotation does not use the container handlers: it is as ready as with every entry (the tests have no game
    // heap, so neither runtime has a slot reader)
    assert!(runtime.moves_ready());
    assert_eq!(runtime.rotation_blocked(), enabled().rotation_blocked());
}

#[test]
fn a_frame_with_no_container_handlers_drops_the_action() {
    let runtime = enabled();
    let screen = chest();
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);

    runtime.run_frame(&screen, Some(0xA), None);

    assert_eq!(runtime.pending_action(), None);
    assert!(!runtime.sort_running());
    assert!(runtime.accept_frame(0xB), "the guard is released");
}

// the container screen draws a second view of the same controller in every frame: both reach `run_frame`
fn both_views(runtime: &HotbarRuntime, screen: &FakeScreen, actions: &mut FakeActions) {
    frame(runtime, screen, 0xA, actions);
    frame(runtime, screen, 0xA, actions);
}

#[test]
fn a_sort_takes_one_step_per_tick_whatever_the_views_render() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::applying(&screen);
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);

    both_views(&runtime, &screen, &mut actions);
    assert_eq!(actions.steps.len(), 1, "the start frame and the second view of the same tick: one step");
    both_views(&runtime, &screen, &mut actions);
    assert_eq!(actions.steps.len(), 1, "more frames of the same tick: still one step");

    runtime.next_tick();
    both_views(&runtime, &screen, &mut actions);
    assert_eq!(actions.steps.len(), 2, "the next tick gives the next step");
}

#[test]
fn a_dump_runs_once_for_one_press_whatever_the_views_render() {
    let runtime = enabled();
    let screen = chest();
    let mut actions = FakeActions::recording();
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::DumpFromContainer, 1.0);

    both_views(&runtime, &screen, &mut actions);
    runtime.next_tick();
    both_views(&runtime, &screen, &mut actions);

    assert_eq!(actions.steps.len(), 2, "the two container stacks, once");
}

#[test]
fn the_put_back_of_a_stuck_sort_runs_once() {
    let runtime = enabled();
    let mut shown = model(2);
    shown.container[1] = stack("apple", 0, 5, 2);
    let screen = FakeScreen::new(ScreenKind::SmallChest, shown);
    let mut actions = FakeActions::applying(&screen);
    runtime.on_top_screen("small_chest_screen", 1.0);
    runtime.on_action(ButtonAction::Sort, 1.0);
    both_views(&runtime, &screen, &mut actions);
    assert_eq!(actions.steps, [ScreenStep::TakeAll(SlotAddr { side: Side::Container, slot: 1 })]);
    // the game fills both slots: the apple on the cursor has no room, and the sort ends Stuck
    {
        let mut shown = screen.model.borrow_mut();
        let shown = shown.as_mut().unwrap();
        shown.container[0] = stack("dirt", 0, 64, 3);
        shown.container[1] = stack("dirt", 0, 64, 4);
    }

    runtime.next_tick();
    both_views(&runtime, &screen, &mut actions);
    runtime.next_tick();
    both_views(&runtime, &screen, &mut actions);

    assert_eq!(
        actions.steps,
        [
            ScreenStep::TakeAll(SlotAddr { side: Side::Container, slot: 1 }),
            ScreenStep::PlaceAll(SlotAddr { side: Side::Player, slot: 9 }),
        ]
    );
    assert!(!runtime.sort_running());
    assert_eq!(screen.held(), None);
}
