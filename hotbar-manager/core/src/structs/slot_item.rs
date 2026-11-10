/// One stack in a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotItem {
    pub id: String,
    pub aux: i16,
    pub count: u8,
    /// The largest stack the game allows for this item.
    pub max: u8,
    /// Two stacks have the same key if and only if the game would merge them.
    pub stack_key: u64,
    pub net_id: i32,
    /// False for a stack Sort must never merge (user data the reader cannot compare).
    pub mergeable: bool,
}
