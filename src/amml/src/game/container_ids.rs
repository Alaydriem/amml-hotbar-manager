use hotbar_manager_core::structs::ScreenKind;

use crate::game::SlotRef;

// the request-protocol container ids, as vanilla sent them on 1.26.62.01 (docs/findings/container-ids/report.md
// section 1). They do not depend on the window id
pub struct ContainerIds;

impl ContainerIds {
    // chests, the ender chest, the hopper, the dispenser and the dropper
    pub const LEVEL_ENTITY: u8 = 7;
    pub const SHULKER_BOX: u8 = 30;
    pub const BARREL: u8 = 58;
    const HOTBAR_SLOTS: u8 = 9;

    // the id of the open container's own slots; None for the player inventory and unknown screens
    pub fn for_screen(kind: ScreenKind) -> Option<u8> {
        match kind {
            ScreenKind::SmallChest
            | ScreenKind::LargeChest
            | ScreenKind::EnderChest
            | ScreenKind::Hopper
            | ScreenKind::Dispenser
            | ScreenKind::Dropper => Some(Self::LEVEL_ENTITY),
            ScreenKind::Barrel => Some(Self::BARREL),
            ScreenKind::ShulkerBox => Some(Self::SHULKER_BOX),
            ScreenKind::PlayerInventory | ScreenKind::Other => None,
        }
    }

    // the container id and slot byte of player slot `slot` (0..8 hotbar, 9..35 main inventory)
    pub fn player(slot: u8) -> (u8, u8) {
        if slot < Self::HOTBAR_SLOTS { (SlotRef::HOTBAR, slot) } else { (SlotRef::INVENTORY, slot) }
    }
}
