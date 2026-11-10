use crate::structs::{ItemMove, MoveError, Side, SlotAddr, SlotItem};

/// A copy of the open inventories. Planners use it to check their plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotModel {
    pub container: Vec<Option<SlotItem>>,
    pub player: [Option<SlotItem>; 36],
}

impl SlotModel {
    fn slot_mut(&mut self, addr: SlotAddr) -> Result<&mut Option<SlotItem>, MoveError> {
        let index = usize::from(addr.slot);
        match addr.side {
            Side::Container => self.container.get_mut(index),
            Side::Player => self.player.get_mut(index),
        }
        .ok_or(MoveError::OutOfRange)
    }

    /// Apply one move. On `Err` the model stays as it was.
    pub fn apply(&mut self, mv: &ItemMove) -> Result<(), MoveError> {
        match *mv {
            ItemMove::Swap { a, b } => self.swap(a, b),
            ItemMove::Place { from, to, count } => self.place(from, to, count),
            ItemMove::Drop { from, count } => self.drop_items(from, count),
        }
    }

    fn swap(&mut self, a: SlotAddr, b: SlotAddr) -> Result<(), MoveError> {
        let item_a = self.slot_mut(a)?.clone().ok_or(MoveError::EmptySlot)?;
        let item_b = self.slot_mut(b)?.clone().ok_or(MoveError::EmptySlot)?;
        if a == b {
            return Ok(());
        }
        *self.slot_mut(a)? = Some(item_b);
        *self.slot_mut(b)? = Some(item_a);
        Ok(())
    }

    fn place(&mut self, from: SlotAddr, to: SlotAddr, count: u8) -> Result<(), MoveError> {
        let source = self.slot_mut(from)?.clone().ok_or(MoveError::EmptySlot)?;
        let target = self.slot_mut(to)?.clone();
        if count == 0 || count > source.count {
            return Err(MoveError::CountTooLarge);
        }
        if from == to {
            return Err(MoveError::KeyMismatch);
        }
        let moved = match target {
            None => {
                if count > source.max {
                    return Err(MoveError::Overfill);
                }
                SlotItem { count, ..source.clone() }
            }
            Some(t) => {
                if t.stack_key != source.stack_key {
                    return Err(MoveError::KeyMismatch);
                }
                let total = u16::from(t.count) + u16::from(count);
                if total > u16::from(t.max) {
                    return Err(MoveError::Overfill);
                }
                SlotItem { count: t.count + count, ..t }
            }
        };
        *self.slot_mut(to)? = Some(moved);
        let left = source.count - count;
        *self.slot_mut(from)? = (left > 0).then_some(SlotItem { count: left, ..source });
        Ok(())
    }

    fn drop_items(&mut self, from: SlotAddr, count: u8) -> Result<(), MoveError> {
        let slot = self.slot_mut(from)?;
        let source = slot.as_ref().ok_or(MoveError::EmptySlot)?;
        if count == 0 || count > source.count {
            return Err(MoveError::CountTooLarge);
        }
        let left = source.count - count;
        if left == 0 {
            *slot = None;
        } else if let Some(item) = slot.as_mut() {
            item.count = left;
        }
        Ok(())
    }
}
