use alaydriem_hotbar_manager::services::SlotConverter;
use amml_api::ItemSnapshot;
use hotbar_manager_core::structs::ScreenKind;

fn snapshot(name: &str, count: u8, identity: Option<u64>) -> ItemSnapshot {
    ItemSnapshot {
        type_name: Some(format!("minecraft:{name}")),
        count,
        aux: Some(0),
        max_count: Some(64),
        identity,
        ..ItemSnapshot::default()
    }
}

fn empty(count: usize) -> Vec<Option<ItemSnapshot>> {
    vec![None; count]
}

#[test]
fn two_stacks_with_one_identity_get_one_key_and_merge() {
    let a = SlotConverter::slot(Some(&snapshot("stone", 3, Some(11)))).unwrap().unwrap();
    let b = SlotConverter::slot(Some(&snapshot("stone", 9, Some(11)))).unwrap().unwrap();

    assert_eq!(a.stack_key, b.stack_key);
    assert!(a.mergeable && b.mergeable);
    assert_eq!((a.id.as_str(), a.aux, a.count, a.max), ("minecraft:stone", 0, 3, 64));
}

#[test]
fn another_identity_aux_or_id_gives_another_key() {
    let base = SlotConverter::stack_key("minecraft:stone", 0, 11);

    assert_ne!(base, SlotConverter::stack_key("minecraft:stone", 0, 12));
    assert_ne!(base, SlotConverter::stack_key("minecraft:stone", 1, 11));
    assert_ne!(base, SlotConverter::stack_key("minecraft:dirt", 0, 11));
}

#[test]
fn a_stack_with_no_identity_has_its_own_key_and_is_never_merged() {
    let named = SlotConverter::slot(Some(&snapshot("diamond_sword", 1, None))).unwrap().unwrap();

    assert!(!named.mergeable);
    assert_eq!(named.stack_key, SlotConverter::unread_key("minecraft:diamond_sword", 0));
    assert_ne!(named.stack_key, SlotConverter::stack_key("minecraft:diamond_sword", 0, 0));
}

#[test]
fn an_empty_slot_and_a_count_of_zero_are_empty() {
    assert_eq!(SlotConverter::slot(None), Some(None));
    assert_eq!(SlotConverter::slot(Some(&snapshot("stone", 0, Some(1)))), Some(None));
}

#[test]
fn a_stack_without_a_name_an_aux_or_a_max_cannot_be_read() {
    let mut no_name = snapshot("stone", 1, Some(1));
    no_name.type_name = None;
    let mut no_aux = snapshot("stone", 1, Some(1));
    no_aux.aux = None;
    let mut no_max = snapshot("stone", 1, Some(1));
    no_max.max_count = None;

    for broken in [no_name, no_aux, no_max] {
        assert_eq!(SlotConverter::slot(Some(&broken)), None);
    }
}

#[test]
fn one_unreadable_stack_makes_the_whole_list_unreadable() {
    let mut no_max = snapshot("stone", 1, Some(1));
    no_max.max_count = None;

    assert_eq!(SlotConverter::slots(&[None, Some(no_max)]), None);
}

#[test]
fn the_model_joins_the_hotbar_and_the_inventory_into_the_player_slots() {
    let mut hotbar = empty(9);
    hotbar[2] = Some(snapshot("stone", 5, Some(1)));
    let mut inventory = empty(27);
    inventory[0] = Some(snapshot("dirt", 7, Some(2)));
    let container = vec![Some(snapshot("gold_ingot", 2, Some(3))), None, None, None, None];

    let model = SlotConverter::model(ScreenKind::Hopper, &container, &hotbar, &inventory).unwrap();

    assert_eq!(model.container.len(), 5);
    assert_eq!(model.container[0].as_ref().map(|item| item.count), Some(2));
    assert_eq!(model.player[2].as_ref().map(|item| item.id.as_str()), Some("minecraft:stone"));
    assert_eq!(model.player[9].as_ref().map(|item| item.id.as_str()), Some("minecraft:dirt"));
    assert!(model.player[10].is_none());
}

#[test]
fn a_collection_of_the_wrong_length_gives_no_model() {
    assert!(SlotConverter::model(ScreenKind::SmallChest, &empty(27), &empty(9), &empty(27)).is_some());
    assert!(SlotConverter::model(ScreenKind::SmallChest, &empty(54), &empty(9), &empty(27)).is_none());
    assert!(SlotConverter::model(ScreenKind::SmallChest, &empty(27), &empty(8), &empty(27)).is_none());
    assert!(SlotConverter::model(ScreenKind::SmallChest, &empty(27), &empty(9), &empty(26)).is_none());
    assert!(SlotConverter::model(ScreenKind::Other, &empty(0), &empty(9), &empty(27)).is_none());
}

#[test]
fn the_player_inventory_has_no_container_slots() {
    assert_eq!(SlotConverter::container_slots(ScreenKind::PlayerInventory), Some(0));
    assert_eq!(SlotConverter::container_slots(ScreenKind::LargeChest), Some(54));
    assert_eq!(SlotConverter::container_slots(ScreenKind::Dropper), Some(9));
    assert_eq!(SlotConverter::container_slots(ScreenKind::Other), None);
}

#[test]
fn the_player_list_needs_exactly_36_slots() {
    assert!(SlotConverter::player(&empty(36)).is_some());
    assert!(SlotConverter::player(&empty(35)).is_none());
    assert!(SlotConverter::player(&empty(37)).is_none());
}
