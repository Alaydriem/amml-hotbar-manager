// the raw values the game gives for one non-empty slot, before they become a `SlotItem`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSlot {
    pub id: String,
    pub aux: i16,
    pub count: u8,
    pub max: u8,
    pub net_id: i32,
    // the serialized user-data NBT of the stack; empty for a stack without user data
    pub user_data: Vec<u8>,
    // the stack has user data that could not be serialized: `user_data` is empty and the stack gets a key of its own
    pub unread_user_data: bool,
}
