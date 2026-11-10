use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ItemMove, MoveError, Side, SlotAddr, SlotItem};

fn item(id: &str, key: u64, count: u8, max: u8) -> SlotItem {
    SlotItem { id: id.to_string(), aux: 0, count, max, stack_key: key, net_id: 1, mergeable: true }
}

fn player(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Player, slot }
}

fn model() -> SlotModel {
    SlotModel { container: Vec::new(), player: std::array::from_fn(|_| None) }
}

#[test]
fn slot_model_rejects_merge_of_different_keys() {
    let mut m = model();
    m.player[0] = Some(item("minecraft:stone", 1, 10, 64));
    m.player[1] = Some(item("minecraft:dirt", 2, 10, 64));
    let mv = ItemMove::Place { from: player(0), to: player(1), count: 5 };
    assert!(m.apply(&mv).is_err());
    assert_eq!(m.player[0].as_ref().map(|i| i.count), Some(10));
    assert_eq!(m.player[1].as_ref().map(|i| i.count), Some(10));
}

#[test]
fn slot_model_rejects_overfill() {
    let mut m = model();
    m.player[0] = Some(item("minecraft:ender_pearl", 1, 10, 16));
    m.player[1] = Some(item("minecraft:ender_pearl", 1, 10, 16));
    let mv = ItemMove::Place { from: player(0), to: player(1), count: 10 };
    assert_eq!(m.apply(&mv), Err(MoveError::Overfill));
}

#[test]
fn slot_model_merges_same_key_within_max() {
    let mut m = model();
    m.player[0] = Some(item("minecraft:stone", 1, 10, 64));
    m.player[1] = Some(item("minecraft:stone", 1, 10, 64));
    let mv = ItemMove::Place { from: player(0), to: player(1), count: 10 };
    assert_eq!(m.apply(&mv), Ok(()));
    assert!(m.player[0].is_none());
    assert_eq!(m.player[1].as_ref().map(|i| i.count), Some(20));
}

#[test]
fn slot_model_rejects_swap_with_empty_side() {
    let mut m = model();
    m.player[0] = Some(item("minecraft:stone", 1, 10, 64));
    let mv = ItemMove::Swap { a: player(0), b: player(1) };
    assert_eq!(m.apply(&mv), Err(MoveError::EmptySlot));
}

#[test]
fn slot_model_rejects_drop_of_more_than_count() {
    let mut m = model();
    m.player[0] = Some(item("minecraft:stone", 1, 10, 64));
    let mv = ItemMove::Drop { from: player(0), count: 11 };
    assert_eq!(m.apply(&mv), Err(MoveError::CountTooLarge));
    let ok = ItemMove::Drop { from: player(0), count: 10 };
    assert_eq!(m.apply(&ok), Ok(()));
    assert!(m.player[0].is_none());
}

#[test]
fn slot_model_reaches_container_slots() {
    let mut m = model();
    m.container = vec![Some(item("minecraft:stone", 1, 3, 64)), None];
    let mv = ItemMove::Place { from: SlotAddr { side: Side::Container, slot: 0 }, to: player(4), count: 3 };
    assert_eq!(m.apply(&mv), Ok(()));
    assert!(m.container[0].is_none());
    assert_eq!(m.player[4].as_ref().map(|i| i.count), Some(3));
    let bad = ItemMove::Drop { from: SlotAddr { side: Side::Container, slot: 9 }, count: 1 };
    assert_eq!(m.apply(&bad), Err(MoveError::OutOfRange));
}
