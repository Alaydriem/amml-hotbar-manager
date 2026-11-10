use std::cell::{Cell, RefCell};

use alaydriem_hotbar_manager::traits::ScreenSource;
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ScreenKind, ScreenStep, Side, SlotAddr, SlotItem};

// a screen stand-in: shows a model and a cursor the test can change, counts the reads, and applies a step as the game
// would (`apply`)
pub struct FakeScreen {
    pub kind: ScreenKind,
    pub model: RefCell<Option<SlotModel>>,
    // None: the cursor cannot be read; Some(None): it is empty
    pub cursor: RefCell<Option<Option<SlotItem>>>,
    pub reads: Cell<usize>,
}

impl FakeScreen {
    pub fn new(kind: ScreenKind, model: SlotModel) -> Self {
        Self { kind, model: RefCell::new(Some(model)), cursor: RefCell::new(Some(None)), reads: Cell::new(0) }
    }

    pub fn hold(&self, item: Option<SlotItem>) {
        *self.cursor.borrow_mut() = Some(item);
    }

    pub fn held(&self) -> Option<SlotItem> {
        self.cursor.borrow().clone().flatten()
    }

    pub fn slot(&self, addr: SlotAddr) -> Option<SlotItem> {
        let model = self.model.borrow();
        let model = model.as_ref()?;
        match addr.side {
            Side::Container => model.container.get(usize::from(addr.slot)).cloned().flatten(),
            Side::Player => model.player.get(usize::from(addr.slot)).cloned().flatten(),
        }
    }

    // the step as the game does it: take, place (merge or exchange), swap, drop. An auto-place is not modelled
    pub fn apply(&self, step: ScreenStep) {
        let mut model = self.model.borrow_mut();
        let Some(model) = model.as_mut() else { return };
        let mut cursor = self.cursor.borrow_mut();
        let Some(cursor) = cursor.as_mut() else { return };
        match step {
            ScreenStep::TakeAll(a) => {
                if cursor.is_none() {
                    *cursor = slot_mut(model, a).take();
                }
            }
            ScreenStep::PlaceAll(a) => {
                let there = slot_mut(model, a);
                match (there.take(), cursor.take()) {
                    (None, held) => *there = held,
                    (Some(mut stack), Some(mut held)) if stack.stack_key == held.stack_key => {
                        let moved = held.count.min(stack.max - stack.count);
                        stack.count += moved;
                        held.count -= moved;
                        *there = Some(stack);
                        *cursor = (held.count > 0).then_some(held);
                    }
                    (Some(stack), Some(held)) => {
                        *there = Some(held);
                        *cursor = Some(stack);
                    }
                    (stack, None) => *there = stack,
                }
            }
            ScreenStep::Swap(a, b) => {
                let first = slot_mut(model, a).take();
                let second = std::mem::replace(slot_mut(model, b), first);
                *slot_mut(model, a) = second;
            }
            ScreenStep::DropSlot(a) => *slot_mut(model, a) = None,
            ScreenStep::DropCursor => *cursor = None,
            ScreenStep::AutoPlace(_) => {}
        }
    }
}

fn slot_mut(model: &mut SlotModel, addr: SlotAddr) -> &mut Option<SlotItem> {
    match addr.side {
        Side::Container => &mut model.container[usize::from(addr.slot)],
        Side::Player => &mut model.player[usize::from(addr.slot)],
    }
}

impl ScreenSource for FakeScreen {
    fn kind(&self) -> ScreenKind {
        self.kind
    }

    fn read(&self) -> Option<SlotModel> {
        self.reads.set(self.reads.get() + 1);
        self.model.borrow().clone()
    }

    fn cursor(&self) -> Option<Option<SlotItem>> {
        self.cursor.borrow().clone()
    }
}

// a stack of `count` of item `name`, whose stack key is the name's length plus its aux (enough to tell the fakes apart)
pub fn stack(name: &str, aux: i16, count: u8, net_id: i32) -> Option<SlotItem> {
    let stack_key = name.bytes().map(u64::from).sum::<u64>() * 100 + u64::try_from(aux).unwrap_or(0);
    Some(SlotItem { id: format!("minecraft:{name}"), aux, count, max: 64, stack_key, net_id, mergeable: true })
}

pub fn model(container_slots: usize) -> SlotModel {
    SlotModel { container: vec![None; container_slots], player: std::array::from_fn(|_| None) }
}
