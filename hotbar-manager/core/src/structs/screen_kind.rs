/// The kind of screen that is open, as far as the button layout cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenKind {
    SmallChest,
    LargeChest,
    Barrel,
    ShulkerBox,
    EnderChest,
    Hopper,
    Dispenser,
    Dropper,
    PlayerInventory,
    Other,
}

impl ScreenKind {
    /// True for every container screen; false for the player inventory and unknown screens.
    pub fn sortable(self) -> bool {
        !matches!(self, Self::PlayerInventory | Self::Other)
    }
}
