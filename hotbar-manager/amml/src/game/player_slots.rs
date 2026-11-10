use std::ffi::c_void;

use amml_game::{GameEntries, ItemStackBaseRef, RawObject};
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ScreenKind, SlotItem};

use crate::game::PlayerInventory;
use crate::services::SlotReader;
use crate::traits::ScreenSource;
use crate::types::PlayerCalls;

// The player's 36 slots as the local player's `FillingContainer` holds them, read through the slot reader. The hidden
// inventory open of the rotation has no container screen controller to read, so this is its source. It reads exactly
// what `PlayerPredictor` swaps. Built for one tick; it keeps nothing. Game thread only
pub struct PlayerSlots<'a> {
    entries: &'a GameEntries,
    reader: &'a SlotReader,
    calls: PlayerCalls,
    client: *mut c_void,
}

impl<'a> PlayerSlots<'a> {
    const SLOTS: usize = 36;

    /// the slots of `client`'s local player
    ///
    /// # Safety
    /// `client` must be null or the live `ClientInstance`, of the build `entries` and `calls` were resolved for, on the
    /// game thread, and stay live while the value is used
    #[allow(unsafe_code)]
    pub unsafe fn new(
        entries: &'a GameEntries,
        reader: &'a SlotReader,
        calls: PlayerCalls,
        client: *mut c_void,
    ) -> Self {
        Self { entries, reader, calls, client }
    }

    #[allow(unsafe_code)]
    fn inventory(&self) -> Option<PlayerInventory> {
        if self.client.is_null() {
            return None;
        }
        // SAFETY: the client is live (`new`), whose vtable holds getLocalPlayer at the entry's slot
        let function = unsafe { RawObject::slot(self.client, self.calls.local_player_slot) }?;
        // SAFETY: the slot holds `LocalPlayer* getLocalPlayer() const`
        let player = unsafe { RawObject::call_pointer(function, self.client) }?;
        // SAFETY: the client's live local player
        unsafe { PlayerInventory::of_player(player, &self.calls) }
    }
}

impl ScreenSource for PlayerSlots<'_> {
    fn kind(&self) -> ScreenKind {
        ScreenKind::PlayerInventory
    }

    #[allow(unsafe_code)]
    fn read(&self) -> Option<SlotModel> {
        let inventory = self.inventory()?;
        let slots = (0..Self::SLOTS)
            .map(|slot| {
                let layout = inventory.item(slot)?;
                // SAFETY: a slot of the live item vector, an ItemStack of the build the entries belong to
                let stack = unsafe {
                    ItemStackBaseRef::from_raw(std::ptr::from_ref(layout).cast_mut().cast::<c_void>(), self.entries)
                }?;
                self.reader.raw_stack(&stack)
            })
            .collect::<Option<Vec<_>>>()?;
        SlotReader::model(Vec::new(), slots)
    }

    // the player's inventory has no cursor: the rotation never reads it
    fn cursor(&self) -> Option<Option<SlotItem>> {
        None
    }
}
