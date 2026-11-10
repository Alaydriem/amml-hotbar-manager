use super::Collection;

/// The inventory that holds a slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Container,
    /// Slots 0-8 are the hotbar; slots 9-35 are the main inventory.
    Player,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotAddr {
    pub side: Side,
    pub slot: u8,
}

impl SlotAddr {
    /// The game collection that holds this slot, and the index inside it.
    pub fn collection(self) -> (Collection, i32) {
        let slot = i32::from(self.slot);
        match self.side {
            Side::Container => (Collection::Container, slot),
            Side::Player if slot < 9 => (Collection::Hotbar, slot),
            Side::Player => (Collection::Inventory, slot - 9),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemMove {
    /// Exchange two full stacks. Both slots must hold an item.
    Swap { a: SlotAddr, b: SlotAddr },
    /// Move `count` items into an empty slot, or onto a stack with the same `stack_key`.
    Place { from: SlotAddr, to: SlotAddr, count: u8 },
    /// Throw `count` items out of a slot.
    Drop { from: SlotAddr, count: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    OutOfRange,
    EmptySlot,
    CountTooLarge,
    KeyMismatch,
    Overfill,
}
