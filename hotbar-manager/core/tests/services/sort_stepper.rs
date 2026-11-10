use std::collections::BTreeMap;

use hotbar_manager_core::services::{SortStepper, SortTick, StuckReason};
use hotbar_manager_core::structs::{ScreenStep, Side, SlotAddr, SlotItem};

fn c(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Container, slot }
}

fn stack(id: &str, count: u8, key: u64) -> Option<SlotItem> {
    sized(id, count, 64, key)
}

fn sized(id: &str, count: u8, max: u8, key: u64) -> Option<SlotItem> {
    Some(SlotItem { id: format!("minecraft:{id}"), aux: 0, count, max, stack_key: key, net_id: 1, mergeable: true })
}

fn pad(mut items: Vec<Option<SlotItem>>, size: usize) -> Vec<Option<SlotItem>> {
    items.resize(size, None);
    items
}

/// The container and the cursor as the game shows them.
struct Screen {
    container: Vec<Option<SlotItem>>,
    cursor: Option<SlotItem>,
}

impl Screen {
    fn new(container: Vec<Option<SlotItem>>) -> Self {
        Self { container, cursor: None }
    }

    fn apply(&mut self, step: ScreenStep) {
        match step {
            ScreenStep::TakeAll(a) => {
                if self.cursor.is_none() {
                    self.cursor = self.container[usize::from(a.slot)].take();
                }
            }
            ScreenStep::PlaceAll(a) => {
                let slot = &mut self.container[usize::from(a.slot)];
                match (slot.take(), self.cursor.take()) {
                    (None, held) => *slot = held,
                    (Some(mut there), Some(mut held)) if there.stack_key == held.stack_key => {
                        assert!(there.mergeable && held.mergeable, "a place onto an unmergeable stack");
                        let moved = held.count.min(there.max - there.count);
                        there.count += moved;
                        held.count -= moved;
                        *slot = Some(there);
                        self.cursor = (held.count > 0).then_some(held);
                    }
                    (Some(there), Some(held)) => {
                        *slot = Some(held);
                        self.cursor = Some(there);
                    }
                    (there, None) => *slot = there,
                }
            }
            ScreenStep::Swap(a, b) => self.container.swap(usize::from(a.slot), usize::from(b.slot)),
            other => panic!("the sort must not use {other:?}"),
        }
    }

    fn tick(&mut self, stepper: &mut SortStepper) -> SortTick {
        let tick = stepper.next(&self.container, self.cursor.as_ref());
        if let SortTick::Step(step) = tick {
            self.apply(step);
        }
        tick
    }

    fn totals(&self) -> BTreeMap<(String, i16), u32> {
        let mut totals = BTreeMap::new();
        for item in self.container.iter().chain([&self.cursor]).flatten() {
            *totals.entry((item.id.clone(), item.aux)).or_insert(0) += u32::from(item.count);
        }
        totals
    }
}

/// Run to the end; check the cursor is empty and no item is lost.
fn run(container: Vec<Option<SlotItem>>) -> (Screen, Vec<ScreenStep>) {
    let mut screen = Screen::new(container);
    let before = screen.totals();
    let mut stepper = SortStepper::new(screen.container.len());
    let mut steps = Vec::new();
    for _ in 0..1000 {
        match screen.tick(&mut stepper) {
            SortTick::Step(step) => steps.push(step),
            SortTick::Done => {
                assert!(screen.cursor.is_none());
                assert_eq!(screen.totals(), before);
                return (screen, steps);
            }
            SortTick::Stuck => panic!("stuck after {steps:?}"),
        }
    }
    panic!("no end");
}

fn ids(screen: &Screen) -> Vec<Option<(String, u8)>> {
    screen.container.iter().map(|s| s.as_ref().map(|i| (i.id.replace("minecraft:", ""), i.count))).collect()
}

fn some(id: &str, count: u8) -> Option<(String, u8)> {
    Some((id.to_string(), count))
}

#[test]
fn a_sorted_container_is_done_at_once() {
    let mut screen = Screen::new(pad(vec![stack("apple", 5, 1), stack("dirt", 5, 2)], 27));
    let mut stepper = SortStepper::new(27);

    assert_eq!(screen.tick(&mut stepper), SortTick::Done);
}

#[test]
fn part_stacks_merge_through_take_then_place() {
    let start = pad(vec![stack("stone", 10, 1), None, None, stack("stone", 20, 1)], 27);
    let mut screen = Screen::new(start.clone());
    let mut stepper = SortStepper::new(27);

    assert_eq!(stepper.next(&screen.container, None), SortTick::Step(ScreenStep::TakeAll(c(3))));
    screen.apply(ScreenStep::TakeAll(c(3)));
    assert_eq!(stepper.next(&screen.container, screen.cursor.as_ref()), SortTick::Step(ScreenStep::PlaceAll(c(0))));

    let (done, _) = run(start);
    assert_eq!(ids(&done)[0], some("stone", 30));
    assert_eq!(done.container.iter().flatten().count(), 1);
}

#[test]
fn an_overflowing_merge_puts_the_rest_back() {
    let mut screen = Screen::new(pad(vec![stack("stone", 50, 1), stack("stone", 30, 1)], 27));
    let mut stepper = SortStepper::new(27);

    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::TakeAll(c(1))));
    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::PlaceAll(c(0))));
    assert_eq!(screen.cursor.as_ref().map(|i| i.count), Some(16));
    // the rest of a merge is still the stack the sort took
    assert!(stepper.holds(screen.cursor.as_ref()));
    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::PlaceAll(c(1))));
    assert_eq!(screen.tick(&mut stepper), SortTick::Done);
}

#[test]
fn out_of_order_filled_slots_swap_in_one_step() {
    let mut screen = Screen::new(pad(vec![stack("dirt", 5, 2), stack("apple", 5, 1)], 27));
    let mut stepper = SortStepper::new(27);

    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::Swap(c(0), c(1))));
    assert_eq!(screen.tick(&mut stepper), SortTick::Done);
}

#[test]
fn a_stack_moves_into_an_empty_slot_through_take_then_place() {
    let mut screen = Screen::new(pad(vec![None, None, None, None, None, stack("apple", 5, 1)], 27));
    let mut stepper = SortStepper::new(27);

    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::TakeAll(c(5))));
    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::PlaceAll(c(0))));
    assert_eq!(screen.tick(&mut stepper), SortTick::Done);
}

#[test]
fn a_held_cursor_at_start_gives_no_step() {
    let screen = Screen::new(pad(vec![stack("dirt", 5, 2), stack("apple", 5, 1)], 27));
    let mut stepper = SortStepper::new(27);
    let held = stack("stone", 3, 9);

    assert_eq!(stepper.next(&screen.container, held.as_ref()), SortTick::Stuck);
}

#[test]
fn a_step_that_changes_nothing_ends_stuck_after_three_frames() {
    let container = pad(vec![stack("dirt", 5, 2), stack("apple", 5, 1)], 27);
    let mut stepper = SortStepper::new(27);

    let ticks: Vec<SortTick> = (0..4).map(|_| stepper.next(&container, None)).collect();

    assert!(matches!(ticks[0], SortTick::Step(_)));
    assert!(matches!(ticks[1], SortTick::Step(_)));
    assert!(matches!(ticks[2], SortTick::Step(_)));
    assert_eq!(ticks[3], SortTick::Stuck);
    assert_eq!(stepper.stuck_reason(), Some(StuckReason::Unchanged));
}

#[test]
fn the_step_limit_ends_a_long_sort() {
    // two different unsorted layouts, so no frame looks unchanged; the limit is 4 x 1 slot
    let first = pad(vec![stack("dirt", 5, 2), stack("apple", 5, 1)], 3);
    let second = pad(vec![stack("dirt", 5, 2), None, stack("apple", 5, 1)], 3);
    let mut stepper = SortStepper::new(1);

    for n in 0..4 {
        let container = if n % 2 == 0 { &first } else { &second };
        assert!(matches!(stepper.next(container, None), SortTick::Step(_)), "step {n}");
    }

    assert_eq!(stepper.next(&first, None), SortTick::Stuck);
    assert_eq!(stepper.stuck_reason(), Some(StuckReason::StepLimit));
    assert_eq!(stepper.steps(), 4);
}

#[test]
fn unmergeable_stacks_are_ordered_but_never_merged() {
    let plain = |id: &str| SlotItem { mergeable: false, ..stack(id, 1, 7).unwrap() };
    let (done, _) = run(pad(vec![Some(plain("stone")), stack("apple", 3, 1), Some(plain("stone"))], 27));

    assert_eq!(ids(&done)[..4], [some("apple", 3), some("stone", 1), some("stone", 1), None]);
}

#[test]
fn a_mixed_container_sorts_with_all_totals_kept() {
    let (done, steps) = run(pad(
        vec![
            stack("stone", 10, 1),
            None,
            stack("dirt", 4, 2),
            stack("stone", 60, 1),
            stack("apple", 1, 3),
            None,
            stack("dirt", 40, 2),
            stack("stone", 5, 1),
        ],
        27,
    ));

    assert!(!steps.is_empty());
    assert_eq!(ids(&done)[..6], [some("apple", 1), some("dirt", 44), some("stone", 64), some("stone", 11), None, None]);
}

#[test]
fn a_full_stack_is_not_a_merge_source() {
    // zz max 16, aa max 64: a full aa stack behind a part-stack is ordered by a swap, not by a 3-step bounce
    let (done, steps) = run(pad(
        vec![
            sized("zz", 14, 16, 2),
            sized("zz", 11, 16, 2),
            stack("aa", 47, 3),
            stack("aa", 64, 3),
            stack("aa", 60, 3),
        ],
        5,
    ));

    assert!(steps.len() <= 20, "{} steps", steps.len());
    assert_eq!(ids(&done), [some("aa", 64), some("aa", 64), some("aa", 43), some("zz", 16), some("zz", 9)]);
}

#[test]
fn small_containers_end_done_inside_the_step_limit() {
    // a fixed pseudo-random fill of 5 and 9 slots with mixed max sizes; `run` panics on Stuck
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut roll = move |n: u64| {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (seed >> 33) % n
    };
    for slots in [1usize, 2, 3, 5, 9] {
        for _ in 0..300 {
            let container: Vec<Option<SlotItem>> = (0..slots)
                .map(|_| {
                    let kind = roll(4);
                    let max = [64u8, 16, 1, 64][kind as usize];
                    let count = 1 + roll(u64::from(max)) as u8;
                    (roll(3) != 0)
                        .then(|| sized(["aa", "bb", "cc", "dd"][kind as usize], count, max, kind + 1))
                        .flatten()
                })
                .collect();
            let (_, steps) = run(container);
            assert!(steps.len() <= slots * 4);
        }
    }
}

#[test]
fn the_limit_never_leaves_the_stack_on_the_cursor() {
    // limit 4: step 4 is a take, so the cursor holds a stack when the limit is spent
    let mut screen = Screen::new(pad(
        vec![
            sized("zz", 14, 16, 2),
            sized("zz", 11, 16, 2),
            stack("aa", 47, 3),
            stack("aa", 64, 3),
            stack("aa", 60, 3),
        ],
        5,
    ));
    let before = screen.totals();
    let mut stepper = SortStepper::new(1);
    let mut steps = 0;
    let mut held_at_limit = false;
    loop {
        match screen.tick(&mut stepper) {
            SortTick::Step(_) => {
                steps += 1;
                held_at_limit |= steps == 4 && screen.cursor.is_some();
            }
            SortTick::Stuck => break,
            SortTick::Done => panic!("the sort cannot end inside 4 steps"),
        }
        assert!(steps < 50);
    }

    assert!(held_at_limit);
    assert!(steps > 4);
    assert!(screen.cursor.is_none());
    assert_eq!(screen.totals(), before);
}

#[test]
fn a_place_never_puts_down_a_stack_the_sort_did_not_take() {
    let mut screen = Screen::new(pad(vec![None, None, None, None, None, stack("apple", 5, 1)], 27));
    let mut stepper = SortStepper::new(27);
    assert!(!stepper.holding());
    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::TakeAll(c(5))));
    assert!(stepper.holding());
    assert!(stepper.holds(screen.cursor.as_ref()));
    // the game put another stack on the cursor (same id and count, another aux)
    screen.cursor = Some(SlotItem { aux: 1, ..stack("apple", 5, 1).unwrap() });

    assert_eq!(stepper.next(&screen.container, screen.cursor.as_ref()), SortTick::Stuck);
    assert_eq!(stepper.stuck_reason(), Some(StuckReason::ForeignCursor));
    assert!(!stepper.holds(screen.cursor.as_ref()));
    assert_eq!(StuckReason::ForeignCursor.text(), "the cursor holds a stack the sort did not take");
}

#[test]
fn a_cursor_stack_with_another_key_is_not_the_sorts_own() {
    let mut screen = Screen::new(pad(vec![None, stack("apple", 5, 1)], 27));
    let mut stepper = SortStepper::new(27);
    assert_eq!(screen.tick(&mut stepper), SortTick::Step(ScreenStep::TakeAll(c(1))));
    screen.cursor = stack("apple", 5, 99);

    assert_eq!(stepper.next(&screen.container, screen.cursor.as_ref()), SortTick::Stuck);
    assert_eq!(stepper.stuck_reason(), Some(StuckReason::ForeignCursor));
}

#[test]
fn a_held_cursor_at_start_names_the_foreign_cursor() {
    let mut stepper = SortStepper::new(2);

    assert_eq!(stepper.next(&pad(vec![], 2), stack("stone", 3, 9).as_ref()), SortTick::Stuck);
    assert_eq!(stepper.stuck_reason(), Some(StuckReason::ForeignCursor));
    assert_eq!(stepper.steps(), 0);
}

#[test]
fn no_room_for_the_held_stack_names_it() {
    // one slot: the sort takes the only stack, then the game fills the slot with another item
    let mut stepper = SortStepper::new(2);
    let start = vec![None, stack("apple", 5, 1)];
    assert_eq!(stepper.next(&start, None), SortTick::Step(ScreenStep::TakeAll(c(1))));
    let full = vec![stack("dirt", 64, 2), stack("dirt", 64, 2)];

    assert_eq!(stepper.next(&full, stack("apple", 5, 1).as_ref()), SortTick::Stuck);
    assert_eq!(stepper.stuck_reason(), Some(StuckReason::NoRoom));
    assert!(stepper.holds(stack("apple", 5, 1).as_ref()));
}
