use hotbar_manager_core::services::{RotationPlanner, SlotModel};
use hotbar_manager_core::structs::{Direction, SlotItem};

type Inv = [Option<SlotItem>; 36];

fn item(n: u64) -> SlotItem {
    SlotItem {
        id: format!("minecraft:item_{n}"),
        aux: 0,
        count: 1 + (n % 60) as u8,
        max: 64,
        stack_key: n,
        net_id: n as i32,
        mergeable: true,
    }
}

fn full() -> Inv {
    std::array::from_fn(|i| Some(item(i as u64 + 1)))
}

fn mixed() -> Inv {
    std::array::from_fn(|i| if i % 3 == 0 || i == 10 { None } else { Some(item(i as u64 + 1)) })
}

fn rotate(inv: &Inv, dir: Direction) -> Inv {
    let mut m = SlotModel { container: Vec::new(), player: inv.clone() };
    for mv in RotationPlanner::plan(inv, dir) {
        assert_eq!(m.apply(&mv), Ok(()));
    }
    m.player
}

fn keys(inv: &Inv) -> Vec<Option<u64>> {
    inv.iter().map(|s| s.as_ref().map(|i| i.stack_key)).collect()
}

#[test]
fn rotate_down_moves_row3_into_hotbar() {
    let inv = full();
    let out = rotate(&inv, Direction::Down);
    for c in 0..9 {
        let id = |s: &Inv, r: usize| s[r * 9 + c].as_ref().map(|i| i.stack_key);
        assert_eq!(id(&out, 0), id(&inv, 3));
        assert_eq!(id(&out, 1), id(&inv, 0));
        assert_eq!(id(&out, 2), id(&inv, 1));
        assert_eq!(id(&out, 3), id(&inv, 2));
    }
}

#[test]
fn rotate_up_moves_row1_into_hotbar() {
    let inv = full();
    let out = rotate(&inv, Direction::Up);
    for c in 0..9 {
        let id = |s: &Inv, r: usize| s[r * 9 + c].as_ref().map(|i| i.stack_key);
        assert_eq!(id(&out, 0), id(&inv, 1));
        assert_eq!(id(&out, 1), id(&inv, 2));
        assert_eq!(id(&out, 2), id(&inv, 3));
        assert_eq!(id(&out, 3), id(&inv, 0));
    }
}

#[test]
fn four_rotations_restore_layout() {
    for dir in [Direction::Down, Direction::Up] {
        for start in [full(), mixed()] {
            let mut cur = start.clone();
            for _ in 0..4 {
                cur = rotate(&cur, dir);
            }
            assert_eq!(keys(&cur), keys(&start));
        }
    }
}

#[test]
fn empty_inventory_plans_nothing() {
    let inv: Inv = std::array::from_fn(|_| None);
    assert!(RotationPlanner::plan(&inv, Direction::Down).is_empty());
    assert!(RotationPlanner::plan(&inv, Direction::Up).is_empty());
}

#[test]
fn every_planned_move_is_valid() {
    for dir in [Direction::Down, Direction::Up] {
        for start in [full(), mixed()] {
            let mut m = SlotModel { container: Vec::new(), player: start.clone() };
            for mv in RotationPlanner::plan(&start, dir) {
                assert_eq!(m.apply(&mv), Ok(()));
            }
        }
    }
}

#[test]
fn identical_stacks_are_never_merged_by_rotation() {
    // two stacks with the same key swap places; counts stay as they were
    let mut inv: Inv = std::array::from_fn(|_| None);
    inv[0] = Some(SlotItem { count: 5, ..item(1) });
    inv[9] = Some(SlotItem { count: 7, ..item(1) });
    let out = rotate(&inv, Direction::Up);
    assert_eq!(out[0].as_ref().map(|i| i.count), Some(7));
    assert_eq!(out[27].as_ref().map(|i| i.count), Some(5));
}
