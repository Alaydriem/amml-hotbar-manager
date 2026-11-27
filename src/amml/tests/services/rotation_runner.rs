use alaydriem_hotbar_manager::services::{RotationEnd, RotationRunner};
use amml_api::{InventoryMove, RequestId, RequestOutcome};
use hotbar_manager_core::structs::{Direction, SlotItem};

fn item(n: u64) -> SlotItem {
    SlotItem {
        id: format!("minecraft:item_{n}"),
        aux: 0,
        count: 1 + (n % 60) as u8,
        max: 64,
        stack_key: n,
        mergeable: true,
    }
}

fn player_with(slots: &[usize]) -> [Option<SlotItem>; 36] {
    let mut player: [Option<SlotItem>; 36] = std::array::from_fn(|_| None);
    for slot in slots {
        player[*slot] = Some(item(*slot as u64 + 1));
    }
    player
}

#[test]
fn a_full_inventory_plans_three_swaps_per_column() {
    let moves = RotationRunner::plan(&player_with(&(0..36).collect::<Vec<_>>()), Direction::Down);

    assert_eq!(moves.len(), 27);
    assert!(moves.iter().all(|mv| matches!(mv, InventoryMove::Swap { .. })));
    assert_eq!(moves[0], InventoryMove::Swap { a: 0, b: 9 });
}

#[test]
fn a_lone_hotbar_stack_moves_whole_into_the_empty_slot_above_it() {
    let moves = RotationRunner::plan(&player_with(&[3]), Direction::Down);

    assert_eq!(moves, [InventoryMove::Place { from: 3, to: 12, count: item(4).count }]);
}

#[test]
fn an_empty_inventory_plans_nothing() {
    assert!(RotationRunner::plan(&player_with(&[]), Direction::Up).is_empty());
}

#[test]
fn only_the_id_of_the_pending_request_ends_the_rotation() {
    let mut runner = RotationRunner::new();
    runner.started(RequestId(5), 4, 100);

    assert!(runner.active());
    assert_eq!(runner.finished(RequestId(6), RequestOutcome::Done, 4, 4), None);
    assert!(runner.active());
    assert_eq!(runner.finished(RequestId(5), RequestOutcome::Done, 4, 4), Some(RotationEnd::Done { moves: 4 }));
    assert!(!runner.active());
    assert_eq!(runner.finished(RequestId(5), RequestOutcome::Done, 4, 4), None);
}

#[test]
fn an_outcome_that_is_not_done_is_an_early_end() {
    let mut runner = RotationRunner::new();
    runner.started(RequestId(1), 9, 0);

    assert_eq!(
        runner.finished(RequestId(1), RequestOutcome::Refused, 3, 9),
        Some(RotationEnd::Early { outcome: RequestOutcome::Refused, accepted: 3, total: 9 })
    );
}

#[test]
fn a_request_that_never_ends_is_dropped_after_the_expiry() {
    let mut runner = RotationRunner::new();
    runner.started(RequestId(2), 6, 10);

    assert_eq!(runner.expire(10 + RotationRunner::EXPIRE_TICKS - 1), None);
    assert!(runner.active());
    assert_eq!(runner.expire(10 + RotationRunner::EXPIRE_TICKS), Some((RequestId(2), 6)));
    assert!(!runner.active());
}

#[test]
fn a_reset_forgets_the_request() {
    let mut runner = RotationRunner::new();
    runner.started(RequestId(2), 6, 0);

    runner.reset();

    assert!(!runner.active());
    assert_eq!(runner.finished(RequestId(2), RequestOutcome::Done, 6, 6), None);
}
