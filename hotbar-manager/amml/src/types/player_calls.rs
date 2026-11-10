use amml_sdk::SdkError;

use crate::traits::GameHost;

// The resolved game code and offsets the player-slot prediction uses: the local player, its inventory and the
// game's own slot swap. Every offset the prediction reads comes from an entry; with any entry missing nothing is
// predicted. The first three entry names are stratum's. `PlayerInventory.mInventory` and `FillingContainer.mItems`
// have no recipe yet (stratum hard-codes 0xB8 and 0x198), so on today's data the prediction is off
#[derive(Clone, Copy, Debug)]
pub struct PlayerCalls {
    // vtable slot of `LocalPlayer* ClientInstance::getLocalPlayer()`
    pub local_player_slot: u64,
    // offset of `Player::mInventory` (a `std::unique_ptr<PlayerInventory>`)
    pub player_inventory: u64,
    // vtable slot of `void FillingContainer::swapSlots(int, int)`
    pub swap_slots_slot: u64,
    // offset of `PlayerInventory::mInventory` (a `std::unique_ptr<Inventory>`)
    pub inventory: u64,
    // offset of `FillingContainer::mItems` (the `std::vector<ItemStack>` that swapSlots indexes)
    pub items: u64,
}

impl PlayerCalls {
    pub const GET_LOCAL_PLAYER: &'static str = "ClientInstance.getLocalPlayer";
    pub const PLAYER_INVENTORY: &'static str = "Player.playerInventory";
    pub const SWAP_SLOTS: &'static str = "FillingContainer.swapSlots";
    // no recipe yet
    pub const INVENTORY: &'static str = "PlayerInventory.mInventory";
    // no recipe yet
    pub const ITEMS: &'static str = "FillingContainer.mItems";
    pub const ENTRIES: [&'static str; 5] =
        [Self::GET_LOCAL_PLAYER, Self::PLAYER_INVENTORY, Self::SWAP_SLOTS, Self::INVENTORY, Self::ITEMS];

    // the calls of the running build; an error names the first entry that is missing
    pub fn resolve<H: GameHost>(host: &H) -> Result<Self, SdkError> {
        Ok(Self {
            local_player_slot: host.resolve(Self::GET_LOCAL_PLAYER)?,
            player_inventory: host.resolve(Self::PLAYER_INVENTORY)?,
            swap_slots_slot: host.resolve(Self::SWAP_SLOTS)?,
            inventory: host.resolve(Self::INVENTORY)?,
            items: host.resolve(Self::ITEMS)?,
        })
    }
}
