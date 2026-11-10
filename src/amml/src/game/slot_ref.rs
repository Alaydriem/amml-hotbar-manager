// One slot of an item stack request: the container id of the request protocol, the slot, and the net id of the stack
// the slot holds (0 for an empty slot). The ids are the captured ones of docs/findings/container-ids/report.md
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotRef {
    pub container: u8,
    pub slot: u8,
    pub net_id: i32,
}

impl SlotRef {
    // the player's hotbar (slots 0..8) and main inventory (slots 9..35, sent with the player slot number itself)
    pub const HOTBAR: u8 = 28;
    pub const INVENTORY: u8 = 29;
    // the carried stack, slot 0
    pub const CURSOR: u8 = 59;
}
