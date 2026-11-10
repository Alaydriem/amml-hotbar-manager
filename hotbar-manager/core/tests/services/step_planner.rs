use hotbar_manager_core::services::{ItemFilter, PLAYER_ALL, PLAYER_MAIN, SlotModel, StepPlanner};
use hotbar_manager_core::structs::{ButtonAction, Collection, ScreenStep, Side, SlotAddr, SlotItem};

fn stack(id: &str, aux: i16, count: u8) -> Option<SlotItem> {
    Some(SlotItem {
        id: format!("minecraft:{id}"),
        aux,
        count,
        max: 64,
        stack_key: u64::from(aux.unsigned_abs()) + 1,
        net_id: 1,
        mergeable: true,
    })
}

fn model(container: usize) -> SlotModel {
    SlotModel { container: vec![None; container], player: std::array::from_fn(|_| None) }
}

fn p(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Player, slot }
}

fn c(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Container, slot }
}

fn filter(id: &str, aux: i16) -> ItemFilter {
    ItemFilter { id: format!("minecraft:{id}"), aux }
}

#[test]
fn dump_from_inventory_auto_places_the_main_rows_then_the_hotbar() {
    let mut m = model(27);
    m.player[9] = stack("stone", 0, 5);
    m.player[12] = stack("dirt", 0, 3);
    m.player[0] = stack("iron_sword", 0, 1);
    m.player[8] = stack("apple", 0, 2);

    let steps = StepPlanner::transfer(ButtonAction::DumpFromInventory, &m, None);

    // inventory_items 0..26 (player 9..35), then hotbar_items 0..8 (player 0..8)
    assert_eq!(
        steps,
        vec![
            ScreenStep::AutoPlace(p(9)),
            ScreenStep::AutoPlace(p(12)),
            ScreenStep::AutoPlace(p(0)),
            ScreenStep::AutoPlace(p(8)),
        ]
    );
}

#[test]
fn dump_from_container_auto_places_each_filled_container_slot() {
    let mut m = model(27);
    m.container[0] = stack("stone", 0, 5);
    m.container[4] = stack("dirt", 0, 3);

    let steps = StepPlanner::transfer(ButtonAction::DumpFromContainer, &m, None);

    assert_eq!(steps, vec![ScreenStep::AutoPlace(c(0)), ScreenStep::AutoPlace(c(4))]);
}

#[test]
fn filtered_dump_from_inventory_uses_main_rows_then_hotbar() {
    let mut m = model(27);
    m.player[20] = stack("stone", 0, 5);
    m.player[3] = stack("stone", 0, 5);
    m.player[9] = stack("stone", 0, 5);
    m.player[10] = stack("dirt", 0, 5);

    let steps = StepPlanner::transfer(ButtonAction::DumpFromInventoryFiltered, &m, Some(&filter("stone", 0)));

    assert_eq!(steps, vec![ScreenStep::AutoPlace(p(9)), ScreenStep::AutoPlace(p(20)), ScreenStep::AutoPlace(p(3))]);
}

#[test]
fn filtered_dump_from_container_takes_only_matches() {
    let mut m = model(27);
    m.container[0] = stack("stone", 0, 5);
    m.container[1] = stack("stone", 1, 5);
    m.container[2] = stack("dirt", 0, 5);
    m.container[6] = stack("stone", 0, 9);

    let steps = StepPlanner::transfer(ButtonAction::DumpFromContainerFiltered, &m, Some(&filter("stone", 0)));

    assert_eq!(steps, vec![ScreenStep::AutoPlace(c(0)), ScreenStep::AutoPlace(c(6))]);
}

#[test]
fn drop_all_drops_the_main_rows_only() {
    let mut m = model(0);
    m.player[0] = stack("stone", 0, 5);
    m.player[9] = stack("stone", 0, 5);
    m.player[35] = stack("dirt", 0, 5);

    let steps = StepPlanner::transfer(ButtonAction::DropAll, &m, None);

    assert_eq!(steps, vec![ScreenStep::DropSlot(p(9)), ScreenStep::DropSlot(p(35))]);
}

#[test]
fn drop_filtered_drops_the_cursor_first_then_matches() {
    let mut m = model(0);
    m.player[0] = stack("stone", 0, 5);
    m.player[9] = stack("stone", 0, 5);
    m.player[10] = stack("dirt", 0, 5);

    let steps = StepPlanner::transfer(ButtonAction::DropFiltered, &m, Some(&filter("stone", 0)));

    assert_eq!(steps, vec![ScreenStep::DropCursor, ScreenStep::DropSlot(p(9)), ScreenStep::DropSlot(p(0))]);
}

#[test]
fn transfer_steps_do_not_depend_on_free_room() {
    let mut m = model(3);
    for slot in &mut m.container {
        *slot = stack("stone", 0, 64);
    }
    for slot in &mut m.player {
        *slot = stack("dirt", 0, 64);
    }

    assert_eq!(
        StepPlanner::transfer(ButtonAction::DumpFromContainer, &m, None),
        vec![ScreenStep::AutoPlace(c(0)), ScreenStep::AutoPlace(c(1)), ScreenStep::AutoPlace(c(2))]
    );
    assert_eq!(StepPlanner::transfer(ButtonAction::DumpFromInventory, &m, None).len(), 36);
}

#[test]
fn sort_has_no_transfer_steps() {
    assert!(StepPlanner::transfer(ButtonAction::Sort, &model(27), None).is_empty());
}

#[test]
fn collection_maps_each_side() {
    assert_eq!(p(0).collection(), (Collection::Hotbar, 0));
    assert_eq!(p(8).collection(), (Collection::Hotbar, 8));
    assert_eq!(p(9).collection(), (Collection::Inventory, 0));
    assert_eq!(p(35).collection(), (Collection::Inventory, 26));
    assert_eq!(c(26).collection(), (Collection::Container, 26));
}

#[test]
fn collection_names_are_the_game_names() {
    assert_eq!(Collection::Container.name(), "container_items");
    assert_eq!(Collection::Hotbar.name(), "hotbar_items");
    assert_eq!(Collection::Inventory.name(), "inventory_items");
}

#[test]
fn constants_have_the_documented_order() {
    assert_eq!(PLAYER_MAIN[0], 9);
    assert_eq!(PLAYER_MAIN[26], 35);
    assert_eq!(PLAYER_ALL[0], 9);
    assert_eq!(PLAYER_ALL[26], 35);
    assert_eq!(PLAYER_ALL[27], 0);
    assert_eq!(PLAYER_ALL[35], 8);
}
