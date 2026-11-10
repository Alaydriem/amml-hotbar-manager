use amml_sdk::SdkError;

use crate::traits::GameHost;

// The resolved game values the slot reader needs beyond the SDK's own entries. With any of the first three missing
// the reader is off (see `SlotReader::bind`). `ItemStack.mNetIdVariant` and `CompoundTag.write` have no recipe yet (see
// the task 10 report)
#[derive(Clone, Copy, Debug)]
pub struct SlotReadCalls {
    // the address of `uchar ItemStackBase::getMaxStackSize() const`
    pub max_stack_size: usize,
    // offset of `ItemStack::mNetIdVariant` (0x80 in the SDK's `ItemStackLayout`)
    pub net_id: u64,
    // the address of the `ItemStack` vtable: an object whose first qword is not this is not an `ItemStack`, and its
    // net id is never read
    pub item_stack_vtable: usize,
    // the function that serializes a `CompoundTag`; optional. The reader does not call it yet: its signature is not
    // known until a recipe names the function, so a stack with user data gets a key of its own (see
    // `SlotReader::unread_key`)
    pub user_data_write: Option<usize>,
}

impl SlotReadCalls {
    pub const MAX_STACK_SIZE: &'static str = "ItemStackBase.getMaxStackSize";
    // no recipe yet
    pub const NET_ID: &'static str = "ItemStack.mNetIdVariant";
    // has a recipe (`ItemStack.vtable`, kind global, in item_stack.hcl)
    pub const ITEM_STACK_VTABLE: &'static str = "ItemStack.vtable";
    // no recipe yet, and not needed by `resolve`
    pub const USER_DATA_WRITE: &'static str = "CompoundTag.write";
    pub const ENTRIES: [&'static str; 4] =
        [Self::MAX_STACK_SIZE, Self::NET_ID, Self::ITEM_STACK_VTABLE, Self::USER_DATA_WRITE];

    // the calls of the running build; an error names the first required entry that is missing
    pub fn resolve<H: GameHost>(host: &H) -> Result<Self, SdkError> {
        Ok(Self {
            max_stack_size: host.resolve(Self::MAX_STACK_SIZE)? as usize,
            net_id: host.resolve(Self::NET_ID)?,
            item_stack_vtable: host.resolve(Self::ITEM_STACK_VTABLE)? as usize,
            user_data_write: host.resolve(Self::USER_DATA_WRITE).ok().map(|value| value as usize),
        })
    }
}
