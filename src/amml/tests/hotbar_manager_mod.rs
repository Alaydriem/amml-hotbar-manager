use alaydriem_hotbar_manager::HotbarManagerMod;
use amm_client::test_support::FakeTable;
use amml_api::amml_abi::{AmmlInputMode, AmmlRequestOutcome, ClientTick, InventoryRequestDone, UiPointer};
use amml_api::testing::{FakeGame, FakeInventory, FakeItem, FakeScreen};
use amml_api::{Area, InputMode, Point, RequestOptions, Screen, ScreenKind, TextureReload, UiProfile, ViewId};

use crate::support::hotbar_rig::{HotbarRig, ROTATE_DOWN, ROTATE_UP, SORT, item, screen};
use crate::support::screen_applier::ScreenApplier;

const VIEW: u64 = 11;
const SECOND_VIEW: u64 = 12;
const SCREEN: u64 = 5;
// the centre of the drop-filtered button of the player inventory: the panel is at (200, 100), the button at (0, -15)
// with 30 by 15 units
const DROP_FILTERED: (f32, f32) = (215.0, 92.5);

fn inventory_screen() -> FakeScreen {
    screen(ScreenKind::PlayerInventory, 0)
}

// a small chest whose container holds `stacks`, in order from slot 0
fn chest_with(stacks: &[(&'static str, u8, u64)]) -> FakeScreen {
    let mut chest = screen(ScreenKind::SmallChest, 27);
    for (slot, (name, count, identity)) in stacks.iter().enumerate() {
        chest.slots.container[slot] = item(name, *count, *identity);
    }
    chest
}

fn names(chest: &FakeScreen) -> Vec<&'static str> {
    chest.slots.container.iter().flatten().map(|stack| stack.type_name).collect()
}

fn hover(mut fake: FakeScreen, at: (f32, f32)) -> FakeScreen {
    fake.pointer = Some(Point { x: at.0, y: at.1 });
    fake
}

#[test]
fn the_mod_enables_with_every_required_row_served() {
    let rig = HotbarRig::new();

    assert!(rig.game.missing_required().is_empty(), "{:?}", rig.game.missing_required());
}

#[test]
fn enable_fails_while_a_required_area_is_not_served() {
    let game = HotbarRig::offered();
    game.withdraw_area(Area::INVENTORY_REQUEST);
    FakeTable::provide(&game.ctx()).unwrap();
    game.load::<HotbarManagerMod>().unwrap();

    assert!(game.enable::<HotbarManagerMod>().is_err());
}

#[test]
fn the_link_to_the_manager_is_logged_once_at_a_tick() {
    let rig = HotbarRig::new();
    rig.tick();
    rig.tick();

    assert_eq!(rig.count_logged("info hotbar linked"), 1);
}

#[test]
fn disable_removes_the_listeners_and_leaves_the_manager() {
    let rig = HotbarRig::new();

    rig.game.disable::<HotbarManagerMod>();
    let before = rig.game.logs().len();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());

    assert_eq!(rig.game.logs().len(), before);
    assert!(rig.game.draws().is_empty());
    assert!(FakeTable::calls().contains(&"unregister"));
}

// --- buttons ---

#[test]
fn the_buttons_of_the_player_inventory_are_drawn_once_their_textures_are_ready() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);

    rig.render(VIEW, SCREEN, &inventory_screen());
    assert!(rig.game.draws().is_empty(), "a texture that is not ready is skipped");
    assert_eq!(rig.game.textures().loads().len(), 11);
    rig.textures_ready(1..=11, 0);
    rig.render(VIEW, SCREEN, &inventory_screen());

    // two buttons, each with 9 state pieces, 9 border pieces and one glyph
    assert_eq!(rig.game.draws().len(), 2 * 19);
    assert!(rig.game.draws().iter().all(|line| line.starts_with("image ")));
}

#[test]
fn the_first_frame_logs_the_kind_and_where_each_button_sits() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);

    rig.render(VIEW, SCREEN, &inventory_screen());
    rig.render(VIEW, SCREEN, &inventory_screen());

    assert!(rig.logged("info buttons drawn kind=PlayerInventory profile=Classic buttons=2"));
    assert!(rig.logged("info button placed kind=PlayerInventory action=DropFiltered rx=0.3359 ry=0.2569"));
    assert_eq!(rig.count_logged("info buttons drawn"), 1);
}

#[test]
fn the_textures_are_asked_for_once_and_logged_when_ready() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    rig.render(VIEW, SCREEN, &inventory_screen());
    assert_eq!(rig.game.textures().loads().len(), 11);
    assert_eq!(rig.count_logged("info textures ready"), 0);

    rig.textures_ready(1..=11, 0);
    rig.render(VIEW, SCREEN, &inventory_screen());
    rig.render(VIEW, SCREEN, &inventory_screen());

    assert_eq!(rig.count_logged("info textures ready count=11 paints="), 1);
    assert_eq!(rig.game.textures().loads().len(), 11);
}

#[test]
fn the_pocket_profile_has_no_buttons() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    let mut pocket = inventory_screen();
    pocket.ui_profile = Some(UiProfile::Pocket);

    rig.render(VIEW, SCREEN, &pocket);

    assert!(rig.game.draws().is_empty());
    assert!(rig.game.textures().loads().is_empty());
    assert!(rig.game.ui_pointer(ViewId(VIEW), 0, true, DROP_FILTERED.0, DROP_FILTERED.1));
}

#[test]
fn a_frame_the_game_left_images_queued_in_draws_nothing_and_keeps_the_layout() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    rig.textures_ready(1..=11, 0);
    rig.render(VIEW, SCREEN, &inventory_screen());
    let drawn = rig.game.draws().len();

    let mut busy = inventory_screen();
    busy.can_draw = false;
    rig.render(VIEW, SCREEN, &busy);

    assert_eq!(rig.game.draws().len(), drawn);
    assert!(!rig.game.ui_pointer(ViewId(VIEW), 0, true, DROP_FILTERED.0, DROP_FILTERED.1));
}

#[test]
fn a_stale_texture_set_is_asked_for_again_after_a_world_leave() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    rig.textures_ready(1..=11, 0);
    rig.render(VIEW, SCREEN, &inventory_screen());

    rig.game.textures().leave_world();
    rig.render(VIEW, SCREEN, &inventory_screen());
    assert_eq!(rig.count_logged("info button textures were stale"), 1);
    assert_eq!(rig.game.textures().loads().len(), 11);
    rig.render(VIEW, SCREEN, &inventory_screen());
    assert_eq!(rig.game.textures().loads().len(), 22);
    rig.textures_ready(12..=22, 1);
    let before = rig.game.draws().len();
    rig.render(VIEW, SCREEN, &inventory_screen());

    assert_eq!(rig.game.draws().len() - before, 2 * 19);
}

#[test]
fn a_device_reset_asks_for_the_texture_set_again_and_logs_ready_again() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    rig.textures_ready(1..=11, 0);
    rig.render(VIEW, SCREEN, &inventory_screen());
    assert_eq!(rig.count_logged("info textures ready"), 1);

    rig.game.raise_textures_reloaded(TextureReload::DeviceReset);
    rig.render(VIEW, SCREEN, &inventory_screen());

    assert!(rig.logged("info textures reloaded reason=device_reset held=11"));
    assert_eq!(rig.count_logged("info button textures were stale"), 0);
    assert_eq!(rig.game.textures().loads().len(), 22);
    rig.textures_ready(12..=22, 1);
    let before = rig.game.draws().len();
    rig.render(VIEW, SCREEN, &inventory_screen());
    assert_eq!(rig.game.draws().len() - before, 2 * 19);
    assert_eq!(rig.count_logged("info textures ready"), 2);
}

#[test]
fn a_resource_pack_change_keeps_the_set_and_logs_ready_again() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    rig.textures_ready(1..=11, 0);
    rig.render(VIEW, SCREEN, &inventory_screen());

    rig.game.raise_textures_reloaded(TextureReload::ResourcePack);
    rig.render(VIEW, SCREEN, &inventory_screen());

    assert!(rig.logged("info textures reloaded reason=resource_pack held=11"));
    assert_eq!(rig.game.textures().loads().len(), 11);
    assert_eq!(rig.count_logged("info textures ready"), 2);
}

#[test]
fn a_reload_before_any_paint_holds_nothing() {
    let rig = HotbarRig::new();

    rig.game.raise_textures_reloaded(TextureReload::DeviceReset);

    assert!(rig.logged("info textures reloaded reason=device_reset held=0"));
    assert!(rig.game.textures().loads().is_empty());
}

// --- clicks ---

#[test]
fn a_press_and_release_on_a_button_is_taken_and_runs_the_action_at_the_next_frame() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    let mut fake = hover(inventory_screen(), DROP_FILTERED);
    fake.cursor = item("minecraft:stone", 5, 1);
    fake.slots.inventory[2] = item("minecraft:stone", 20, 1);
    fake.slots.inventory[3] = item("minecraft:dirt", 20, 2);
    rig.render(VIEW, SCREEN, &fake);
    let view = ViewId(VIEW);

    let down = rig.game.ui_pointer(view, 0, true, DROP_FILTERED.0, DROP_FILTERED.1);
    let up = rig.game.ui_pointer(view, 0, false, DROP_FILTERED.0, DROP_FILTERED.1);

    assert!(!down && !up, "the game gets neither the press nor the release");
    assert!(rig.logged("info pointer taken action=DropFiltered"));
    assert!(rig.game.screen_actions().is_empty(), "nothing is called outside a render");
    rig.render(VIEW, SCREEN, &fake);
    assert!(rig.logged("info action done action=DropFiltered calls=2"), "{:?}", rig.game.logs());
    assert_eq!(rig.game.screen_actions(), ["drop_cursor whole", "drop_slot inventory 2 whole"]);
}

#[test]
fn a_release_off_the_button_does_nothing_but_is_still_swallowed() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    let fake = hover(inventory_screen(), DROP_FILTERED);
    rig.render(VIEW, SCREEN, &fake);
    let view = ViewId(VIEW);

    assert!(!rig.game.ui_pointer(view, 0, true, DROP_FILTERED.0, DROP_FILTERED.1));
    assert!(!rig.game.ui_pointer(view, 0, false, 300.0, 300.0));

    assert_eq!(rig.count_logged("info pointer taken"), 0);
    assert_eq!(rig.count_logged("info action"), 0);
}

#[test]
fn a_click_beside_the_buttons_goes_to_the_game() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    let view = ViewId(VIEW);

    assert!(rig.game.ui_pointer(view, 0, true, 300.0, 200.0));
    assert!(rig.game.ui_pointer(view, 0, false, 300.0, 200.0));
}

#[test]
fn a_click_on_another_view_of_the_screen_goes_to_the_game() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    let mut blank = inventory_screen();
    blank.kind = ScreenKind::Other;
    rig.render(SECOND_VIEW, SCREEN, &blank);
    let view = ViewId(SECOND_VIEW);

    assert!(rig.game.ui_pointer(view, 0, true, DROP_FILTERED.0, DROP_FILTERED.1));
    assert!(rig.game.ui_pointer(view, 0, false, DROP_FILTERED.0, DROP_FILTERED.1));
}

#[test]
fn a_gamepad_press_over_a_button_goes_to_the_game() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &inventory_screen());
    let mut event = UiPointer {
        header: FakeGame::header::<UiPointer>(),
        screen: SCREEN,
        view: VIEW,
        pointer: 0,
        pressed: 1,
        input_mode: AmmlInputMode::from(InputMode::GamePad).0,
        _reserved: 0,
        x: DROP_FILTERED.0,
        y: DROP_FILTERED.1,
        _reserved2: 0,
    };

    let dispatch = rig.game.raise_with_client(&mut event);

    assert!(!dispatch.cancelled);
}

// --- screen actions ---

#[test]
fn the_sort_key_sorts_a_chest_one_step_per_tick() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    let mut chest =
        chest_with(&[("minecraft:stone", 10, 1), ("minecraft:dirt", 10, 2), ("minecraft:cobblestone", 10, 3)]);

    rig.press(SORT);
    let mut seen = 0;
    for _ in 0..60 {
        // the second view of the same screen takes no second step in the tick
        rig.render(VIEW, SCREEN, &chest);
        let first = rig.game.screen_actions().len();
        rig.render(SECOND_VIEW, SCREEN, &chest);
        assert_eq!(rig.game.screen_actions().len(), first, "a second view took a step in the same tick");
        ScreenApplier::apply(&mut chest, &rig.game.screen_actions()[seen..]);
        seen = first;
        if rig.count_logged("info sort done") > 0 {
            break;
        }
        rig.tick();
    }

    assert_eq!(rig.count_logged("info sort started action=Sort"), 1);
    assert_eq!(rig.count_logged("info sort done"), 1, "{:?}", rig.game.logs());
    assert_eq!(names(&chest), ["minecraft:cobblestone", "minecraft:dirt", "minecraft:stone"]);
}

#[test]
fn a_sort_key_over_the_hud_is_ignored() {
    let rig = HotbarRig::new();

    rig.press(SORT);

    assert!(rig.logged("info action ignored action=Sort reason=NoScreen"));
}

#[test]
fn a_dump_runs_all_its_steps_in_its_first_frame() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    // the dump-from-container button is at (-15, 0) of the panel with 15 by 30 units
    let at = (200.0 - 7.5, 115.0);
    let chest = hover(chest_with(&[("minecraft:stone", 10, 1), ("minecraft:dirt", 10, 2)]), at);
    rig.render(VIEW, SCREEN, &chest);
    let view = ViewId(VIEW);

    rig.game.ui_pointer(view, 0, true, at.0, at.1);
    rig.game.ui_pointer(view, 0, false, at.0, at.1);
    rig.render(VIEW, SCREEN, &chest);

    assert!(rig.logged("info action done action=DumpFromContainer calls=2"), "{:?}", rig.game.logs());
    assert_eq!(rig.game.screen_actions(), ["auto_place container 0", "auto_place container 1"]);
}

#[test]
fn a_screen_changed_to_the_hud_stops_a_running_sort() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    let chest = chest_with(&[("minecraft:stone", 10, 1), ("minecraft:dirt", 10, 2)]);
    rig.press(SORT);
    rig.render(VIEW, SCREEN, &chest);
    assert_eq!(rig.count_logged("info sort started"), 1);

    rig.changed(SCREEN, 1, Screen::Hud);

    assert_eq!(rig.count_logged("info sort stopped steps=1 reason=the screen closed or changed"), 1);
}

#[test]
fn a_screen_change_is_logged_with_the_top_screen() {
    let rig = HotbarRig::new();

    rig.changed(1, SCREEN, Screen::Container);
    rig.changed(SCREEN, 1, Screen::Hud);

    assert!(rig.logged("info screen changed screen=Container"));
    assert!(rig.logged("info screen changed screen=Hud"));
}

#[test]
fn a_close_seen_at_a_tick_drops_an_action_that_waits_for_a_frame() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    rig.press(SORT);

    rig.show(Screen::Hud);
    rig.show(Screen::Container);
    rig.render(VIEW, SCREEN, &chest_with(&[("minecraft:stone", 10, 1)]));

    assert_eq!(rig.count_logged("info sort started"), 0);
}

#[test]
fn a_frame_of_another_screen_drops_the_running_sort() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    let chest = chest_with(&[("minecraft:stone", 10, 1), ("minecraft:dirt", 10, 2)]);
    rig.press(SORT);
    rig.render(VIEW, SCREEN, &chest);
    assert_eq!(rig.count_logged("info sort started"), 1);

    rig.render(VIEW, SCREEN + 1, &chest);

    assert_eq!(rig.count_logged("info sort stopped steps=1 reason=the screen closed or changed"), 1);
}

#[test]
fn a_container_that_cannot_be_read_stops_the_sort_with_a_warning() {
    let rig = HotbarRig::new();
    rig.show(Screen::Container);
    let chest = chest_with(&[("minecraft:stone", 10, 1), ("minecraft:dirt", 10, 2)]);
    rig.press(SORT);
    rig.render(VIEW, SCREEN, &chest);
    rig.tick();
    // a small chest whose container list has the wrong length cannot be planned on
    let mut broken = chest.clone();
    broken.slots.container.truncate(5);

    rig.render(VIEW, SCREEN, &broken);

    assert_eq!(rig.count_logged("warn sort stopped"), 1, "{:?}", rig.game.logs());
}

// --- rotation ---

fn rotation_inventory(slots: &[usize]) -> FakeInventory {
    let mut inventory = FakeInventory { selected_slot: Some(0), ..FakeInventory::default() };
    for slot in slots {
        inventory.slots[*slot] = item("minecraft:stone", 4, *slot as u64 + 1);
    }
    inventory
}

#[test]
fn a_rotation_key_gives_the_loader_one_request_with_a_hidden_open_and_a_prediction() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&(0..36).collect::<Vec<_>>()));

    rig.press(ROTATE_DOWN);

    let requests = rig.game.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].1.len(), 27);
    assert_eq!(requests[0].2, RequestOptions { hidden_open: true, predict: true });
}

#[test]
fn the_rotation_ends_with_the_answer_of_its_request() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[0, 1, 2]));
    rig.press(ROTATE_UP);
    let (id, moves, _) = rig.game.requests().remove(0);

    rig.game.answer(id, u16::try_from(moves.len()).unwrap());
    rig.game.frame();

    assert!(rig.logged(&format!("info rotation done moves={}", moves.len())), "{:?}", rig.game.logs());
}

#[test]
fn a_refused_move_logs_an_early_end() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[0, 1, 2]));
    rig.press(ROTATE_UP);
    let (id, _, _) = rig.game.requests().remove(0);

    rig.game.answer(id, 1);
    rig.game.frame();

    assert_eq!(rig.count_logged("warn the row rotation ended early"), 1);
    assert_eq!(rig.count_logged("info rotation done"), 0);
}

#[test]
fn a_second_rotation_key_waits_for_the_first_request() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[0, 1, 2]));
    rig.press(ROTATE_UP);

    rig.press(ROTATE_DOWN);

    assert_eq!(rig.game.requests().len(), 1);
    assert!(rig.logged("info rotation ignored reason=busy"));
}

#[test]
fn the_request_done_of_another_request_does_not_end_the_rotation() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[0, 1, 2]));
    rig.press(ROTATE_UP);
    let (id, moves, _) = rig.game.requests().remove(0);
    let mut other = InventoryRequestDone {
        header: FakeGame::header::<InventoryRequestDone>(),
        id: id.0 + 100,
        outcome: AmmlRequestOutcome::DONE.0,
        _reserved: 0,
        accepted: 1,
        total: 1,
        _reserved2: 0,
    };

    rig.game.raise(&mut other);
    rig.press(ROTATE_DOWN);

    assert_eq!(rig.count_logged("info rotation done"), 0);
    assert!(rig.logged("info rotation ignored reason=busy"));
    rig.game.answer(id, u16::try_from(moves.len()).unwrap());
    rig.game.frame();
    assert_eq!(rig.count_logged("info rotation done"), 1);
}

#[test]
fn a_request_a_screen_cancelled_is_logged_as_a_cancel_and_frees_the_rotation() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[0, 1, 2]));
    rig.press(ROTATE_UP);
    let (id, _, _) = rig.game.requests().remove(0);
    let mut cancelled = InventoryRequestDone {
        header: FakeGame::header::<InventoryRequestDone>(),
        id: id.0,
        outcome: AmmlRequestOutcome::CANCELLED.0,
        _reserved: 0,
        accepted: 0,
        total: 3,
        _reserved2: 0,
    };

    rig.game.raise(&mut cancelled);

    assert_eq!(rig.count_logged("info the row rotation was cancelled"), 1);
    assert_eq!(rig.count_logged("warn"), 0);
    rig.game.answer(id, 3);
    rig.press(ROTATE_DOWN);
    assert_eq!(rig.count_logged("info rotation ignored reason=busy"), 0);
}

#[test]
fn a_rotation_key_with_a_screen_open_does_nothing() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[0, 1, 2]));
    rig.show(Screen::Container);

    rig.press(ROTATE_UP);

    assert!(rig.game.requests().is_empty());
    assert!(rig.logged("info rotation ignored reason=screen_open"));
}

#[test]
fn an_inventory_with_no_stack_logs_a_rotation_of_no_moves() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[]));

    rig.press(ROTATE_UP);

    assert!(rig.game.requests().is_empty());
    assert!(rig.logged("info rotation done moves=0"));
}

#[test]
fn a_stack_the_loader_cannot_describe_stops_the_rotation_before_a_request() {
    let rig = HotbarRig::new();
    let mut inventory = rotation_inventory(&[0, 1]);
    inventory.slots[1] = Some(FakeItem { max_count: None, ..inventory.slots[1].unwrap() });
    rig.game.set_inventory(inventory);

    rig.press(ROTATE_UP);

    assert!(rig.game.requests().is_empty());
    assert!(rig.logged("info rotation ignored reason=no_read"));
}

#[test]
fn a_sort_key_while_a_rotation_runs_is_ignored() {
    let rig = HotbarRig::new();
    rig.game.set_inventory(rotation_inventory(&[0, 1, 2]));
    rig.press(ROTATE_UP);
    rig.show(Screen::Container);

    rig.press(SORT);

    assert!(rig.logged("info action ignored action=Sort reason=Busy"));
}

#[test]
fn a_tick_without_a_client_does_not_panic() {
    let rig = HotbarRig::new();

    let dispatch = rig.game.raise(&mut ClientTick { header: FakeGame::header::<ClientTick>() });

    assert!(!dispatch.panicked);
}
