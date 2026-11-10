use std::ffi::c_void;

use amml_game::{ItemStackLayout, RawObject};

use crate::types::PlayerCalls;

// `void FillingContainer::swapSlots(int, int)`: this, the two slots
type SwapSlotsFn = unsafe extern "C" fn(*mut c_void, i32, i32);

// The local player's `PlayerInventory` and the `FillingContainer` it holds: the stacks as they are, read through the
// `ItemStack` mirror over the container's item vector, and the game's own slot swap. Copied from stratum, trimmed to
// what the prediction uses. Every offset comes from an entry (`PlayerCalls`). Game thread only
pub struct PlayerInventory {
    items: *mut c_void,
    items_offset: u64,
    swap_slots_slot: u64,
}

impl PlayerInventory {
    // the player's Inventory has 36 slots (hotbar and main inventory); an item vector of another size is a misread
    const SLOTS: usize = 36;

    /// the inventory of `player`; None for a null player, inventory or container
    ///
    /// # Safety
    /// `player` must be null or the live local `Player` of the build `calls` was resolved for
    #[allow(unsafe_code)]
    pub unsafe fn of_player(player: *mut c_void, calls: &PlayerCalls) -> Option<Self> {
        if player.is_null() {
            return None;
        }
        // SAFETY: the caller guarantees a live player; the anchor is the offset of its unique_ptr<PlayerInventory>
        let supplies = unsafe { RawObject::pointer(player, calls.player_inventory) }?;
        // SAFETY: a live PlayerInventory, whose unique_ptr<Inventory> sits at the PlayerInventory.mInventory entry
        let items = unsafe { RawObject::pointer(supplies, calls.inventory) }?;
        let inventory = Self { items, items_offset: calls.items, swap_slots_slot: calls.swap_slots_slot };
        inventory.slots().map(|_| inventory)
    }

    // the first stack of the item vector, when the vector holds exactly SLOTS whole stacks; None otherwise
    fn slots(&self) -> Option<usize> {
        // SAFETY: the live container holds mItems (first, last, end) at the FillingContainer.mItems entry
        #[allow(unsafe_code)]
        let [first, last] = unsafe { RawObject::read::<[usize; 2]>(self.items, self.items_offset) }?;
        let bytes = last.checked_sub(first)?;
        let whole = bytes % size_of::<ItemStackLayout>() == 0;
        (first != 0 && whole && bytes / size_of::<ItemStackLayout>() == Self::SLOTS).then_some(first)
    }

    // the stack in `slot`; None for a slot past the container's end or an item vector that does not look sane
    pub fn item(&self, slot: usize) -> Option<&ItemStackLayout> {
        let first = self.slots()?;
        if slot >= Self::SLOTS {
            return None;
        }
        // SAFETY: a slot inside the container's live item vector
        #[allow(unsafe_code)]
        unsafe {
            (first as *const ItemStackLayout).add(slot).as_ref()
        }
    }

    // the net id of the stack in `slot`; None for a slot past the end or a stack that holds a request id
    pub fn net_id(&self, slot: usize) -> Option<i32> {
        self.item(slot)?.net_id_variant.net_id()
    }

    // swaps the stacks of slots `a` and `b` through the game's own swap, which also tells the container's listeners
    pub fn swap_slots(&self, a: i32, b: i32) {
        // SAFETY: the live container, whose vtable holds swapSlots at the entry's slot
        #[allow(unsafe_code)]
        let Some(function) = (unsafe { RawObject::slot(self.items, self.swap_slots_slot) }) else { return };
        // SAFETY: the slot holds `void FillingContainer::swapSlots(int, int)`
        #[allow(unsafe_code)]
        let swap: SwapSlotsFn = unsafe { std::mem::transmute(function) };
        // SAFETY: the live container and two slots the caller read from it
        #[allow(unsafe_code)]
        unsafe {
            swap(self.items, a, b)
        };
    }
}
