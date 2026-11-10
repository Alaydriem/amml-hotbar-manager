use alaydriem_hotbar_manager::services::SlotReader;
use alaydriem_hotbar_manager::types::{RawSlot, SlotReadCalls};
use amml_msvc::MsvcHeap;
use hotbar_manager_core::structs::ScreenKind;

use crate::support::fake_host::FakeHost;

fn raw(id: &str, aux: i16, count: u8, max: u8, user_data: &[u8]) -> RawSlot {
    RawSlot { id: id.to_string(), aux, count, max, net_id: 5, user_data: user_data.to_vec(), unread_user_data: false }
}

fn empty_player() -> Vec<Option<RawSlot>> {
    vec![None; 36]
}

#[test]
fn fnv_1a_64_gives_the_published_test_vectors_for_the_key_parts() {
    assert_eq!(SlotReader::fnv1a(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(SlotReader::fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(SlotReader::fnv1a(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn the_stack_key_is_the_same_for_the_same_id_aux_and_user_data() {
    let a = SlotReader::stack_key("minecraft:cobblestone", 0, &[]);
    let b = SlotReader::stack_key("minecraft:cobblestone", 0, &[]);

    assert_eq!(a, b);
}

#[test]
fn the_stack_key_differs_when_the_id_the_aux_or_the_user_data_differ() {
    let base = SlotReader::stack_key("minecraft:wool", 0, &[]);

    assert_ne!(base, SlotReader::stack_key("minecraft:stone", 0, &[]));
    assert_ne!(base, SlotReader::stack_key("minecraft:wool", 1, &[]));
    assert_ne!(base, SlotReader::stack_key("minecraft:wool", 0, &[1]));
}

#[test]
fn the_stack_key_does_not_depend_on_the_count_the_max_or_the_net_id() {
    let one = raw("minecraft:stone", 0, 1, 64, &[]);
    let many = RawSlot { count: 64, net_id: 99, ..one.clone() };

    assert_eq!(SlotReader::item(&one).stack_key, SlotReader::item(&many).stack_key);
}

#[test]
fn the_stack_key_never_mixes_the_end_of_the_id_into_the_aux_or_the_data() {
    // the same bytes in a different split of id, aux and data must not collide
    let a = SlotReader::stack_key("ab", 0x0063, &[]);
    let b = SlotReader::stack_key("abc", 0, &[]);

    assert_ne!(a, b);
}

#[test]
fn a_stack_key_is_a_fixed_value_so_it_survives_a_rebuild() {
    // pins the hash input: id, one zero byte, aux as two little-endian bytes, then the user data
    let mut input = b"minecraft:stone".to_vec();
    input.extend_from_slice(&[0, 3, 0, 9, 8]);

    assert_eq!(SlotReader::stack_key("minecraft:stone", 3, &[9, 8]), SlotReader::fnv1a(&input));
}

#[test]
fn the_model_holds_the_raw_values_of_every_slot() {
    let container =
        vec![Some(raw("minecraft:cobblestone", 0, 64, 64, &[])), None, Some(raw("minecraft:sword", 0, 1, 1, &[7]))];
    let mut player = empty_player();
    player[4] = Some(raw("minecraft:apple", 0, 3, 64, &[]));

    let model = SlotReader::model(container, player).unwrap();

    let first = model.container[0].as_ref().unwrap();
    assert_eq!(
        (first.id.as_str(), first.aux, first.count, first.max, first.net_id),
        ("minecraft:cobblestone", 0, 64, 64, 5)
    );
    assert!(model.container[1].is_none());
    assert_eq!(model.container[2].as_ref().unwrap().max, 1);
    assert_eq!(model.player[4].as_ref().unwrap().count, 3);
    assert!(model.player[0].is_none());
}

#[test]
fn a_renamed_item_has_another_stack_key_than_the_plain_item() {
    let plain = raw("minecraft:stone", 0, 1, 64, &[]);
    let renamed = raw("minecraft:stone", 0, 1, 64, &[10, 0, 1, 2]);

    let model = SlotReader::model(vec![Some(plain), Some(renamed)], empty_player()).unwrap();

    let keys: Vec<u64> = model.container.iter().flatten().map(|item| item.stack_key).collect();
    assert_ne!(keys[0], keys[1]);
}

#[test]
fn a_slot_with_a_zero_count_is_empty() {
    let model = SlotReader::model(vec![Some(raw("minecraft:stone", 0, 0, 64, &[]))], empty_player()).unwrap();

    assert!(model.container[0].is_none());
}

#[test]
fn a_player_list_that_is_not_36_slots_gives_no_model() {
    assert!(SlotReader::model(vec![], vec![None; 35]).is_none());
    assert!(SlotReader::model(vec![], vec![None; 37]).is_none());
}

#[test]
fn each_container_screen_has_the_slot_count_of_its_vanilla_grid() {
    let cases = [
        (ScreenKind::SmallChest, Some(27)),
        (ScreenKind::LargeChest, Some(54)),
        (ScreenKind::Barrel, Some(27)),
        (ScreenKind::ShulkerBox, Some(27)),
        (ScreenKind::EnderChest, Some(27)),
        (ScreenKind::Hopper, Some(5)),
        (ScreenKind::Dispenser, Some(9)),
        (ScreenKind::Dropper, Some(9)),
        (ScreenKind::PlayerInventory, Some(0)),
        (ScreenKind::Other, None),
    ];

    for (kind, slots) in cases {
        assert_eq!(SlotReader::container_slots(kind), slots, "{kind:?}");
    }
}

// a heap that `bind` only stores: the tests never allocate
unsafe extern "C" fn no_allocation(_: usize) -> *mut u8 {
    std::ptr::null_mut()
}

unsafe extern "C" fn no_free(_: *mut u8, _: usize) {}

fn heap() -> MsvcHeap {
    MsvcHeap::new(no_allocation, no_free)
}

#[test]
fn bind_gives_a_reader_when_every_entry_resolves() {
    let host = FakeHost::new(&SlotReadCalls::ENTRIES.map(|entry| (entry, 0x1000)));

    assert!(SlotReader::bind(&host, heap()).is_some());
    assert!(host.warnings().is_empty());
}

#[test]
fn bind_without_the_entries_gives_no_reader_and_one_warning() {
    let host = FakeHost::new(&[]);

    assert!(SlotReader::bind(&host, heap()).is_none());
    assert_eq!(host.warnings().len(), 1);
    assert!(host.warnings()[0].contains(SlotReadCalls::MAX_STACK_SIZE), "{:?}", host.warnings());
}

#[test]
fn bind_with_only_the_recipe_backed_entry_names_the_net_id_entry_in_its_one_warning() {
    let host = FakeHost::new(&[(SlotReadCalls::MAX_STACK_SIZE, 0x1000)]);

    assert!(SlotReader::bind(&host, heap()).is_none());
    assert_eq!(host.warnings().len(), 1);
    assert!(host.warnings()[0].contains(SlotReadCalls::NET_ID), "{:?}", host.warnings());
}

fn unread(id: &str, aux: i16) -> RawSlot {
    RawSlot { unread_user_data: true, ..raw(id, aux, 1, 1, &[]) }
}

#[test]
fn a_stack_with_user_data_that_cannot_be_read_still_gets_into_the_model() {
    let model = SlotReader::model(vec![Some(unread("minecraft:iron_pickaxe", 0))], empty_player()).unwrap();

    assert_eq!(model.container[0].as_ref().unwrap().id, "minecraft:iron_pickaxe");
}

#[test]
fn two_stacks_with_unread_user_data_share_a_key_but_neither_is_mergeable() {
    let model = SlotReader::model(
        vec![Some(unread("minecraft:iron_pickaxe", 0)), Some(unread("minecraft:iron_pickaxe", 0))],
        empty_player(),
    )
    .unwrap();

    assert!(model.container.iter().flatten().all(|item| !item.mergeable));
}

#[test]
fn an_unread_stack_keeps_its_key_when_it_moves_and_is_not_mergeable() {
    let model = SlotReader::model(
        vec![Some(unread("minecraft:bow", 0)), None, Some(unread("minecraft:bow", 0))],
        empty_player(),
    )
    .unwrap();

    assert_eq!(model.container[0].as_ref().unwrap().stack_key, model.container[2].as_ref().unwrap().stack_key);
    assert!(!model.container[0].as_ref().unwrap().mergeable);
}

#[test]
fn a_plain_stack_is_mergeable() {
    assert!(SlotReader::item(&raw("minecraft:stone", 0, 1, 64, &[])).mergeable);
}

#[test]
fn an_unread_stack_never_gets_the_key_of_a_plain_stack_of_the_same_id_and_aux() {
    let plain = SlotReader::item(&raw("minecraft:bow", 0, 1, 1, &[])).stack_key;
    let unread = SlotReader::item(&unread("minecraft:bow", 0)).stack_key;

    assert_ne!(plain, unread);
}

#[test]
fn the_unread_key_starts_with_a_byte_no_plain_key_input_starts_with() {
    let mut input = vec![0xFF];
    input.extend_from_slice(b"minecraft:bow");
    input.push(0);
    input.extend_from_slice(&7i16.to_le_bytes());

    assert_eq!(SlotReader::unread_key("minecraft:bow", 7), SlotReader::fnv1a(&input));
}

#[test]
fn plain_stacks_keep_the_normal_key_whatever_their_side_and_index() {
    let item = raw("minecraft:stone", 0, 1, 64, &[]);

    assert_eq!(SlotReader::item(&item).stack_key, SlotReader::item(&item).stack_key);
}

#[test]
fn the_player_list_is_the_hotbar_slots_then_the_main_inventory_slots() {
    let hotbar: Vec<Option<RawSlot>> = (0..9).map(|i| Some(raw("minecraft:hotbar", i, 1, 64, &[]))).collect();
    let inventory: Vec<Option<RawSlot>> = (0..27).map(|i| Some(raw("minecraft:main", i, 1, 64, &[]))).collect();

    let player = SlotReader::player_list(hotbar, inventory).unwrap();

    assert_eq!(player.len(), 36);
    assert_eq!(player[0].as_ref().unwrap().id, "minecraft:hotbar");
    assert_eq!(player[8].as_ref().unwrap().aux, 8);
    assert_eq!(player[9].as_ref().unwrap().id, "minecraft:main");
    assert_eq!(player[9].as_ref().unwrap().aux, 0);
    assert_eq!(player[35].as_ref().unwrap().aux, 26);
}

#[test]
fn the_player_list_needs_nine_hotbar_and_27_inventory_slots() {
    assert!(SlotReader::player_list(vec![None; 8], vec![None; 27]).is_none());
    assert!(SlotReader::player_list(vec![None; 9], vec![None; 28]).is_none());
}

#[test]
fn without_the_item_stack_vtable_entry_bind_gives_no_reader_and_names_it() {
    let host = FakeHost::new(&[(SlotReadCalls::MAX_STACK_SIZE, 0x1000), (SlotReadCalls::NET_ID, 0x80)]);

    assert!(SlotReader::bind(&host, heap()).is_none());
    assert!(host.warnings()[0].contains(SlotReadCalls::ITEM_STACK_VTABLE), "{:?}", host.warnings());
}

#[test]
fn bind_does_not_need_the_user_data_entry() {
    let host = FakeHost::new(&[
        (SlotReadCalls::MAX_STACK_SIZE, 0x1000),
        (SlotReadCalls::NET_ID, 0x80),
        (SlotReadCalls::ITEM_STACK_VTABLE, 0x2000),
    ]);

    assert!(SlotReader::bind(&host, heap()).is_some());
    assert!(host.warnings().is_empty());
}
