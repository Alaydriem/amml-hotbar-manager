use crate::structs::{ScreenStep, Side, SlotAddr, SlotItem};

use super::SortPlanner;

/// What the sort does on one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortTick {
    Step(ScreenStep),
    /// The container matches its target and the cursor is empty.
    Done,
    /// The sort ends without a full order: the step limit is spent (the cursor is empty then), a step
    /// changed nothing for 3 frames in a row, the cursor holds a stack the sort did not take, or no slot
    /// can take the stack the sort holds. In the last three cases the cursor can still hold a stack; the
    /// caller must put it away (for example into the player inventory).
    Stuck,
}

/// Why a sort ended `Stuck`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StuckReason {
    /// The step limit (4 x slots) is spent.
    StepLimit,
    /// A step changed nothing for 3 frames in a row.
    Unchanged,
    /// The cursor holds a stack the sort did not take; the sort never puts it down.
    ForeignCursor,
    /// No container slot can take the stack the sort holds.
    NoRoom,
}

impl StuckReason {
    /// The reason as the log shows it.
    pub fn text(self) -> &'static str {
        match self {
            Self::StepLimit => "the step limit is spent",
            Self::Unchanged => "a step changed nothing for 3 frames",
            Self::ForeignCursor => "the cursor holds a stack the sort did not take",
            Self::NoRoom => "no container slot can take the stack the sort holds",
        }
    }
}

/// A frame with no change after a step: the server may have refused it. Re-issue it this many times.
const UNCHANGED_LIMIT: u32 = 3;

/// One sort, one step per call. The caller reads the screen afresh before each call, so a refused or
/// corrected step shows in the next read and the sort plans again from it.
pub struct SortStepper {
    limit: usize,
    steps: usize,
    unchanged: u32,
    last: Option<(Vec<Option<SlotItem>>, Option<SlotItem>)>,
    // the stack the sort took onto the cursor: its slot, the slot it is meant for, and what it is
    taken: Option<Taken>,
    stuck: Option<StuckReason>,
}

#[derive(Clone)]
struct Taken {
    source: usize,
    dest: usize,
    id: String,
    aux: i16,
    stack_key: u64,
}

impl Taken {
    fn of(item: &SlotItem, source: usize, dest: usize) -> Self {
        Self { source, dest, id: item.id.clone(), aux: item.aux, stack_key: item.stack_key }
    }

    /// The cursor shows the stack the sort took: same id, aux and key. The count can be smaller after a
    /// part merge.
    fn is(&self, held: &SlotItem) -> bool {
        held.id == self.id && held.aux == self.aux && held.stack_key == self.stack_key
    }
}

impl SortStepper {
    pub fn new(slots: usize) -> Self {
        Self { limit: slots.saturating_mul(4), steps: 0, unchanged: 0, last: None, taken: None, stuck: None }
    }

    /// Why the last `next` gave `Stuck`; None when it did not.
    pub fn stuck_reason(&self) -> Option<StuckReason> {
        self.stuck
    }

    /// The steps given so far.
    pub fn steps(&self) -> usize {
        self.steps
    }

    /// Whether the sort took a stack and has not seen it put down yet (the cursor is not read here).
    pub fn holding(&self) -> bool {
        self.taken.is_some()
    }

    /// Whether `cursor` holds the stack the sort took (or the rest of it after a part merge).
    pub fn holds(&self, cursor: Option<&SlotItem>) -> bool {
        matches!((&self.taken, cursor), (Some(taken), Some(held)) if taken.is(held))
    }

    pub fn next(&mut self, container: &[Option<SlotItem>], cursor: Option<&SlotItem>) -> SortTick {
        self.stuck = None;
        match cursor {
            None => {
                self.taken = None;
                if same_layout(container, &SortPlanner::target(container)) {
                    return SortTick::Done;
                }
                // the limit stops a new take or swap only: a stack the sort holds is always put down first
                if self.steps >= self.limit {
                    return self.stop(StuckReason::StepLimit);
                }
            }
            // never put down a stack the sort did not take
            Some(held) if !self.holds(Some(held)) => {
                self.taken = None;
                return self.stop(StuckReason::ForeignCursor);
            }
            Some(_) => {}
        }
        if self.note_frame(container, cursor) {
            return self.stop(StuckReason::Unchanged);
        }
        let step = match cursor {
            Some(held) => self.place(container, held),
            None => self.take_or_swap(container),
        };
        match step {
            Some(step) => {
                self.steps += 1;
                SortTick::Step(step)
            }
            // a take or swap always finds a slot while the layout differs from the target, so only a place fails
            None => self.stop(StuckReason::NoRoom),
        }
    }

    fn stop(&mut self, reason: StuckReason) -> SortTick {
        self.stuck = Some(reason);
        SortTick::Stuck
    }

    /// Record the frame. True when too many frames in a row looked the same.
    fn note_frame(&mut self, container: &[Option<SlotItem>], cursor: Option<&SlotItem>) -> bool {
        let frame = (container.to_vec(), cursor.cloned());
        if self.last.as_ref() == Some(&frame) {
            self.unchanged += 1;
        } else {
            self.unchanged = 0;
        }
        self.last = Some(frame);
        self.unchanged >= UNCHANGED_LIMIT
    }

    /// The cursor holds the stack we took: put it where it was meant to go, or back.
    fn place(&self, container: &[Option<SlotItem>], held: &SlotItem) -> Option<ScreenStep> {
        let taken = self.taken.as_ref()?;
        let room = |there: &SlotItem| joins(there, held) && there.count < there.max;
        let slot = if container.get(taken.dest)?.as_ref().is_none_or(room) {
            Some(taken.dest)
        } else {
            (0..container.len())
                .find(|&i| container[i].as_ref().is_some_and(room))
                .or_else(|| container.get(taken.source).is_some_and(Option::is_none).then_some(taken.source))
                .or_else(|| container.iter().position(Option::is_none))
        };
        slot.map(|i| ScreenStep::PlaceAll(addr(i)))
    }

    /// The cursor is empty: merge a part-stack, or fix the first slot that differs from the target.
    fn take_or_swap(&mut self, container: &[Option<SlotItem>]) -> Option<ScreenStep> {
        for i in 0..container.len() {
            for j in i + 1..container.len() {
                let (Some(dest), Some(src)) = (&container[i], &container[j]) else { continue };
                // both must be part-stacks: merging a full stack only swaps the two counts, which a Swap does in 1 step
                if dest.count < dest.max && src.count < src.max && joins(dest, src) {
                    self.taken = Some(Taken::of(src, j, i));
                    return Some(ScreenStep::TakeAll(addr(j)));
                }
            }
        }
        let target = SortPlanner::target(container);
        let i = (0..container.len()).find(|&i| !same_slot(&container[i], &target[i]))?;
        let wanted = target[i].as_ref()?;
        let j =
            (i + 1..container.len()).find(|&j| container[j].as_ref().is_some_and(|there| same_stack(there, wanted)))?;
        if container[i].is_some() {
            Some(ScreenStep::Swap(addr(i), addr(j)))
        } else {
            self.taken = container[j].as_ref().map(|there| Taken::of(there, j, i));
            Some(ScreenStep::TakeAll(addr(j)))
        }
    }
}

/// Two stacks the game would merge, and the sort may merge.
fn joins(a: &SlotItem, b: &SlotItem) -> bool {
    a.mergeable && b.mergeable && a.stack_key == b.stack_key
}

fn same_stack(a: &SlotItem, b: &SlotItem) -> bool {
    a.id == b.id && a.aux == b.aux && a.count == b.count && a.stack_key == b.stack_key
}

fn same_slot(a: &Option<SlotItem>, b: &Option<SlotItem>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => same_stack(a, b),
        _ => false,
    }
}

fn same_layout(a: &[Option<SlotItem>], b: &[Option<SlotItem>]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_slot(a, b))
}

fn addr(slot: usize) -> SlotAddr {
    SlotAddr { side: Side::Container, slot: u8::try_from(slot).unwrap_or(u8::MAX) }
}
