use super::SlotAddr;

/// The three item collections of the open screen, named as the game names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collection {
    Container,
    Hotbar,
    Inventory,
}

impl Collection {
    /// The collection name the game's container handlers take.
    pub fn name(self) -> &'static str {
        match self {
            Self::Container => "container_items",
            Self::Hotbar => "hotbar_items",
            Self::Inventory => "inventory_items",
        }
    }
}

/// One call of the game's own container handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenStep {
    /// A vanilla shift-click: the game picks the targets.
    AutoPlace(SlotAddr),
    /// Take the whole stack of a slot onto the cursor.
    TakeAll(SlotAddr),
    /// Put the whole cursor stack into a slot.
    PlaceAll(SlotAddr),
    /// Exchange the stacks of two slots.
    Swap(SlotAddr, SlotAddr),
    /// Throw the whole stack of a slot out.
    DropSlot(SlotAddr),
    /// Throw the cursor stack out.
    DropCursor,
}
