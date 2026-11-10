use alaydriem_hotbar_manager::services::ClickRouter;
use alaydriem_hotbar_manager::types::{ButtonLook, ClickRoute, InputMode, UiProfile};
use hotbar_manager_core::services::ButtonLayout;
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, PanelRect, ScreenKind};

// a small chest panel at a half-unit y, as at an odd screen height (docs/findings/spike-align/report.md section 1)
const PANEL: PanelRect = PanelRect { x: 272.0, y: 542.5, w: 176.0, h: 166.0 };
const VIEW: usize = 0x1000;
const OTHER_VIEW: usize = 0x2000;
const LEFT: usize = 0;
const RIGHT: usize = 1;
const MOUSE: Option<InputMode> = Some(InputMode::Mouse);

fn chest() -> Vec<ButtonSpec> {
    ClickRouter::buttons(ScreenKind::SmallChest, UiProfile::Classic)
}

// a point given relative to the panel's top-left
fn at(dx: f32, dy: f32) -> (f32, f32) {
    (PANEL.x + dx, PANEL.y + dy)
}

fn hit(specs: &[ButtonSpec], dx: f32, dy: f32) -> Option<ButtonAction> {
    ClickRouter::hit(specs, PANEL, at(dx, dy))
}

// --- hit test ---

#[test]
fn the_classic_buttons_are_the_layout_of_the_screen() {
    for kind in [ScreenKind::SmallChest, ScreenKind::Hopper, ScreenKind::PlayerInventory] {
        assert_eq!(ClickRouter::buttons(kind, UiProfile::Classic), ButtonLayout::for_screen(kind), "{kind:?}");
    }
}

#[test]
fn a_point_just_inside_each_edge_of_each_chest_button_hits_it() {
    let specs = chest();
    assert_eq!(specs.len(), 5);
    for spec in &specs {
        let (x, y) = spec.offset;
        let (w, h) = spec.size;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        for (dx, dy) in [(x + 0.25, cy), (x + w - 0.25, cy), (cx, y + 0.25), (cx, y + h - 0.25), (x, y)] {
            assert_eq!(hit(&specs, dx, dy), Some(spec.action), "{:?} at ({dx}, {dy})", spec.action);
        }
    }
}

#[test]
fn a_point_just_outside_each_edge_of_each_chest_button_misses_it() {
    let specs = chest();
    for spec in &specs {
        let (x, y) = spec.offset;
        let (w, h) = spec.size;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        // the right and bottom edges are open: x + w belongs to the next button
        for (dx, dy) in
            [(x - 0.25, cy), (x + w, cy), (x + w + 0.25, cy), (cx, y - 0.25), (cx, y + h), (cx, y + h + 0.25)]
        {
            assert_ne!(hit(&specs, dx, dy), Some(spec.action), "{:?} at ({dx}, {dy})", spec.action);
        }
    }
}

#[test]
fn the_dump_column_hits_its_neighbour_across_a_shared_edge_and_nothing_beyond_it() {
    let specs = chest();
    assert_eq!(hit(&specs, -15.25, 15.0), Some(ButtonAction::DumpFromInventory));
    assert_eq!(hit(&specs, -14.75, 15.0), Some(ButtonAction::DumpFromContainer));
    assert_eq!(hit(&specs, -7.5, 29.75), Some(ButtonAction::DumpFromContainer));
    assert_eq!(hit(&specs, -7.5, 30.25), Some(ButtonAction::DumpFromContainerFiltered));
    assert_eq!(hit(&specs, -30.25, 15.0), None);
    assert_eq!(hit(&specs, 0.25, 15.0), None);
    assert_eq!(hit(&specs, -7.5, 60.25), None);
}

#[test]
fn sort_is_hit_at_x_152_5_and_not_at_153_5_where_the_close_button_starts() {
    let specs = chest();
    assert_eq!(hit(&specs, 152.5, 12.5), Some(ButtonAction::Sort));
    assert_eq!(hit(&specs, 153.5, 12.5), None);
    assert_eq!(hit(&specs, 141.5, 12.5), None);
    assert_eq!(hit(&specs, 147.5, 6.5), None);
    assert_eq!(hit(&specs, 147.5, 18.5), None);
}

#[test]
fn the_redstone_buttons_sit_7_units_lower_and_the_inventory_drop_buttons_above_the_panel() {
    let hopper = ClickRouter::buttons(ScreenKind::Hopper, UiProfile::Classic);
    assert_eq!(hit(&hopper, -7.5, 6.75), None);
    assert_eq!(hit(&hopper, -7.5, 7.25), Some(ButtonAction::DumpFromContainerFiltered));
    assert_eq!(hit(&hopper, -22.5, 36.75), Some(ButtonAction::DumpFromInventoryFiltered));
    assert_eq!(hit(&hopper, 152.5, 12.5), Some(ButtonAction::Sort));

    let inventory = ClickRouter::buttons(ScreenKind::PlayerInventory, UiProfile::Classic);
    assert_eq!(hit(&inventory, 15.0, -29.75), Some(ButtonAction::DropAll));
    assert_eq!(hit(&inventory, 15.0, -0.25), Some(ButtonAction::DropFiltered));
    assert_eq!(hit(&inventory, 15.0, 0.25), None);
    assert_eq!(hit(&inventory, 152.5, 12.5), None);
}

#[test]
fn nothing_is_hit_on_other() {
    let specs = ClickRouter::buttons(ScreenKind::Other, UiProfile::Classic);
    assert!(specs.is_empty());
    for (dx, dy) in [(-7.5, 15.0), (152.5, 12.5), (15.0, -20.0), (80.0, 80.0)] {
        assert_eq!(hit(&specs, dx, dy), None, "({dx}, {dy})");
    }
}

#[test]
fn nothing_is_hit_when_the_profile_is_pocket() {
    for kind in [ScreenKind::SmallChest, ScreenKind::LargeChest, ScreenKind::Hopper, ScreenKind::PlayerInventory] {
        let specs = ClickRouter::buttons(kind, UiProfile::Pocket);
        assert!(specs.is_empty(), "{kind:?}");
        for (dx, dy) in [(-7.5, 15.0), (-22.5, 45.0), (152.5, 12.5), (15.0, -20.0)] {
            assert_eq!(hit(&specs, dx, dy), None, "{kind:?} ({dx}, {dy})");
        }
    }
}

// --- press and release ---

#[test]
fn a_press_inside_is_swallowed_and_its_release_inside_fires_once() {
    let mut router = ClickRouter::new();
    let sort = Some(ButtonAction::Sort);

    assert_eq!(router.route(VIEW, LEFT, true, sort, MOUSE), ClickRoute::Swallow);
    assert_eq!(router.route(VIEW, LEFT, false, sort, MOUSE), ClickRoute::Fire(ButtonAction::Sort));
    // nothing is held any more: the next release goes to the game
    assert_eq!(router.route(VIEW, LEFT, false, sort, MOUSE), ClickRoute::Pass);
}

#[test]
fn a_press_and_release_off_every_button_go_to_the_game() {
    let mut router = ClickRouter::new();

    assert_eq!(router.route(VIEW, LEFT, true, None, MOUSE), ClickRoute::Pass);
    assert_eq!(router.route(VIEW, LEFT, false, None, MOUSE), ClickRoute::Pass);
}

#[test]
fn a_swallowed_press_swallows_its_release_off_the_button_and_nothing_fires() {
    let mut router = ClickRouter::new();

    assert_eq!(router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE), ClickRoute::Swallow);
    assert_eq!(router.route(VIEW, LEFT, false, None, MOUSE), ClickRoute::Swallow);
    assert_eq!(router.route(VIEW, LEFT, false, None, MOUSE), ClickRoute::Pass);
}

#[test]
fn a_release_over_another_button_is_swallowed_and_fires_neither() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::DumpFromContainer), MOUSE);
    assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::DumpFromInventory), MOUSE), ClickRoute::Swallow);
}

#[test]
fn a_release_over_a_button_with_no_held_press_goes_to_the_game() {
    let mut router = ClickRouter::new();

    // the press began on a slot and passed; the drag ends over a button
    assert_eq!(router.route(VIEW, LEFT, true, None, MOUSE), ClickRoute::Pass);
    assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Pass);
}

#[test]
fn a_new_press_clears_a_stale_held_press_of_the_same_pointer() {
    let mut router = ClickRouter::new();

    // the release of this press was lost (alt-tab)
    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);
    // a press on a slot: it goes to the game, and its release must too
    assert_eq!(router.route(VIEW, LEFT, true, None, MOUSE), ClickRoute::Pass);
    assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Pass);
}

#[test]
fn each_pointer_has_its_own_held_flag() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);
    // the right button presses and releases on a slot: both go to the game, the left press stays held
    assert_eq!(router.route(VIEW, RIGHT, true, None, MOUSE), ClickRoute::Pass);
    assert_eq!(router.route(VIEW, RIGHT, false, None, MOUSE), ClickRoute::Pass);
    assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Fire(ButtonAction::Sort));
}

#[test]
fn a_right_click_on_a_button_is_swallowed_and_fires_nothing() {
    let mut router = ClickRouter::new();

    assert_eq!(router.route(VIEW, RIGHT, true, Some(ButtonAction::Sort), MOUSE), ClickRoute::Swallow);
    assert_eq!(router.route(VIEW, RIGHT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Swallow);
}

#[test]
fn a_pointer_index_out_of_range_goes_to_the_game() {
    let mut router = ClickRouter::new();

    assert_eq!(router.route(VIEW, ClickRouter::POINTERS, true, Some(ButtonAction::Sort), MOUSE), ClickRoute::Pass);
    assert_eq!(router.route(VIEW, ClickRouter::POINTERS, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Pass);
}

#[test]
fn a_view_change_clears_every_held_flag() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);
    router.route(VIEW, RIGHT, true, Some(ButtonAction::Sort), MOUSE);
    router.observe_view(OTHER_VIEW);

    assert_eq!(router.route(OTHER_VIEW, LEFT, false, None, MOUSE), ClickRoute::Pass);
    assert_eq!(router.route(OTHER_VIEW, RIGHT, false, None, MOUSE), ClickRoute::Pass);
}

#[test]
fn an_event_from_another_view_is_a_view_change() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);

    assert_eq!(router.route(OTHER_VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Pass);
}

#[test]
fn observing_the_same_view_keeps_the_held_flags() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);
    router.observe_view(VIEW);

    assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Fire(ButtonAction::Sort));
}

#[test]
fn forgetting_the_held_view_clears_and_forgetting_another_view_does_not() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);
    router.forget_view(OTHER_VIEW);
    assert_eq!(router.pressed(VIEW, Some(ButtonAction::Sort)), Some(ButtonAction::Sort));

    router.forget_view(VIEW);
    assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Pass);
}

#[test]
fn holds_is_true_only_for_the_view_of_a_held_press() {
    let mut router = ClickRouter::new();
    assert!(!router.holds(VIEW));

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);
    assert!(router.holds(VIEW));
    assert!(!router.holds(OTHER_VIEW));

    router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE);
    assert!(!router.holds(VIEW));
}

#[test]
fn clear_drops_every_held_flag() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);
    router.clear();

    assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), MOUSE), ClickRoute::Pass);
}

// --- input mode ---

#[test]
fn a_gamepad_press_over_a_button_goes_to_the_game() {
    for mode in [InputMode::GamePad, InputMode::MotionController] {
        let mut router = ClickRouter::new();
        assert_eq!(router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), Some(mode)), ClickRoute::Pass, "{mode:?}");
        assert_eq!(router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), Some(mode)), ClickRoute::Pass, "{mode:?}");
    }
}

#[test]
fn a_press_is_taken_in_the_pointer_modes_and_when_the_mode_cannot_be_told() {
    for mode in [Some(InputMode::Mouse), Some(InputMode::Touch), Some(InputMode::Unknown), None] {
        let mut router = ClickRouter::new();
        assert_eq!(router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), mode), ClickRoute::Swallow, "{mode:?}");
        assert_eq!(
            router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), mode),
            ClickRoute::Fire(ButtonAction::Sort)
        );
    }
}

#[test]
fn a_held_press_swallows_its_release_after_a_switch_to_gamepad() {
    let mut router = ClickRouter::new();

    router.route(VIEW, LEFT, true, Some(ButtonAction::Sort), MOUSE);

    assert_eq!(
        router.route(VIEW, LEFT, false, Some(ButtonAction::Sort), Some(InputMode::GamePad)),
        ClickRoute::Fire(ButtonAction::Sort)
    );
}

#[test]
fn the_input_mode_values_map_to_the_enum() {
    assert_eq!(InputMode::from_value(1), InputMode::Mouse);
    assert_eq!(InputMode::from_value(2), InputMode::Touch);
    assert_eq!(InputMode::from_value(3), InputMode::GamePad);
    assert_eq!(InputMode::from_value(4), InputMode::MotionController);
    for value in [0, 5, u32::MAX] {
        assert_eq!(InputMode::from_value(value), InputMode::Unknown, "{value}");
    }
}

// --- look ---

#[test]
fn the_look_is_pressed_only_when_held_and_inside() {
    let mut router = ClickRouter::new();
    let sort = ButtonAction::Sort;
    router.route(VIEW, LEFT, true, Some(sort), MOUSE);

    assert_eq!(router.look(VIEW, sort, Some(sort)), ButtonLook::Pressed);
    assert_eq!(router.pressed(VIEW, Some(sort)), Some(sort));
    // held, but the pointer has left the button
    assert_eq!(router.look(VIEW, sort, None), ButtonLook::Default);
    assert_eq!(router.pressed(VIEW, None), None);
    // held, the pointer is over another button: that one shows hover
    assert_eq!(router.look(VIEW, sort, Some(ButtonAction::DumpFromContainer)), ButtonLook::Default);
    assert_eq!(
        router.look(VIEW, ButtonAction::DumpFromContainer, Some(ButtonAction::DumpFromContainer)),
        ButtonLook::Hover
    );
    assert_eq!(router.pressed(VIEW, Some(ButtonAction::DumpFromContainer)), None);
}

#[test]
fn the_look_is_hover_when_inside_and_not_held_and_default_otherwise() {
    let router = ClickRouter::new();
    let sort = ButtonAction::Sort;

    assert_eq!(router.look(VIEW, sort, Some(sort)), ButtonLook::Hover);
    assert_eq!(router.look(VIEW, sort, None), ButtonLook::Default);
    assert_eq!(router.look(VIEW, sort, Some(ButtonAction::DropAll)), ButtonLook::Default);
}

#[test]
fn another_view_never_shows_the_pressed_look() {
    let mut router = ClickRouter::new();
    let sort = ButtonAction::Sort;
    router.route(VIEW, LEFT, true, Some(sort), MOUSE);

    // the second, taller render of the same screen: hover only, and the held flag stays
    assert_eq!(router.look(OTHER_VIEW, sort, Some(sort)), ButtonLook::Hover);
    assert_eq!(router.pressed(OTHER_VIEW, Some(sort)), None);
    assert_eq!(router.look(VIEW, sort, Some(sort)), ButtonLook::Pressed);
}
