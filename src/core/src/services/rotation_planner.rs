use crate::services::SlotModel;
use crate::structs::{Direction, ItemMove, Side, SlotAddr, SlotItem};

const COLS: u8 = 9;

/// Plans the rotation of the four player rows (hotbar and three main rows).
pub struct RotationPlanner;

impl RotationPlanner {
    /// Per column, a 3-swap cycle. Down: rows (0,1),(0,2),(0,3). Up: (0,1),(1,2),(2,3).
    /// A swap with one empty side becomes a full-stack `Place`; two empty sides emit nothing.
    pub fn plan(player: &[Option<SlotItem>; 36], dir: Direction) -> Vec<ItemMove> {
        let pairs: [(u8, u8); 3] = match dir {
            Direction::Down => [(0, 1), (0, 2), (0, 3)],
            Direction::Up => [(0, 1), (1, 2), (2, 3)],
        };
        let mut model = SlotModel { container: Vec::new(), player: player.clone() };
        let mut plan = Vec::new();
        for column in 0..COLS {
            for (row_a, row_b) in pairs {
                let a = addr(row_a * COLS + column);
                let b = addr(row_b * COLS + column);
                let mv = match (&model.player[usize::from(a.slot)], &model.player[usize::from(b.slot)]) {
                    (None, None) => continue,
                    (Some(_), Some(_)) => ItemMove::Swap { a, b },
                    (Some(item), None) => ItemMove::Place { from: a, to: b, count: item.count },
                    (None, Some(item)) => ItemMove::Place { from: b, to: a, count: item.count },
                };
                // the model is the planner's own state; a failure means a bad input stack
                // (count over max), so skip the move instead of emitting an invalid one
                if model.apply(&mv).is_ok() {
                    plan.push(mv);
                }
            }
        }
        plan
    }
}

fn addr(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Player, slot }
}
