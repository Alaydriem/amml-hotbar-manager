use std::cmp::{Ordering, Reverse};

use crate::structs::SlotItem;

/// The target layout of a container sort. `SortStepper` walks the container to it, one step per frame.
pub struct SortPlanner;

type Layout = Vec<Option<SlotItem>>;

impl SortPlanner {
    /// The sorted layout: part-stacks merged, then ordered. A corrupt container (a stack over its max,
    /// or over 256 slots) keeps its layout.
    pub fn target(container: &[Option<SlotItem>]) -> Layout {
        if !valid(container) {
            return container.to_vec();
        }
        let merged = merge(container);
        let order = sorted_order(&merged);
        order.iter().map(|&i| merged[i].clone()).collect()
    }
}

/// Fill the earliest part-stack of each key from the later ones.
fn merge(container: &[Option<SlotItem>]) -> Layout {
    let mut slots = container.to_vec();
    for i in 0..slots.len() {
        for j in i + 1..slots.len() {
            let (Some(dest), Some(src)) = (&slots[i], &slots[j]) else { continue };
            let room = dest.max.saturating_sub(dest.count);
            if room == 0 {
                break;
            }
            if dest.stack_key != src.stack_key || !dest.mergeable || !src.mergeable {
                continue;
            }
            let count = room.min(src.count);
            let left = src.count - count;
            if let Some(d) = slots[i].as_mut() {
                d.count += count;
            }
            slots[j] = if left == 0 { None } else { slots[j].take().map(|s| SlotItem { count: left, ..s }) };
        }
    }
    slots
}

/// Indices of `slots` in target order. Stable, so ties keep input order.
fn sorted_order(slots: &[Option<SlotItem>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..slots.len()).collect();
    order.sort_by(|&x, &y| match (&slots[x], &slots[y]) {
        (Some(a), Some(b)) => (&a.id, Reverse(a.count), a.aux).cmp(&(&b.id, Reverse(b.count), b.aux)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
    order
}

/// False for a corrupt container: over 256 slots, or a stack above its max.
fn valid(container: &[Option<SlotItem>]) -> bool {
    container.len() <= 256 && container.iter().flatten().all(|item| item.count <= item.max)
}
