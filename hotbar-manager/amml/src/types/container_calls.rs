use amml_sdk::SdkError;

use crate::traits::GameHost;

// The resolved game values of the vanilla container screen handlers the screen actions call (sort, dump, drop). All of
// them are needed: with any one missing the screen actions are off, with one warning that names it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerCalls {
    // the address of `void ContainerScreenController::_handleAutoPlace(this, int amount, std::string const*, int)`
    pub auto_place: usize,
    // the address of `void ContainerScreenController::_handleTakeAll(this, std::string const*, int)`
    pub take_all: usize,
    // the address of `void ContainerScreenController::_handleDropItem(this, std::string const*, int,
    // ItemTransferAmount const*)`
    pub drop_item: usize,
    // the address of `void ContainerScreenController::_handleDropSelectedItem(this, ItemTransferAmount const*)`
    pub drop_selected: usize,
    // vtable slot of `void ContainerScreenController::_handlePlaceAll(this, std::string const*, int)`
    pub place_all_slot: u64,
    // offset of `ContainerScreenController::mContainerManagerController` (a shared_ptr; its object pointer is first)
    pub manager: u64,
    // vtable slot of `bool ContainerManagerController::handleSwap(this, SlotData const*, SlotData const*)`
    pub swap_slot: u64,
}

impl ContainerCalls {
    pub const AUTO_PLACE: &'static str = "ContainerScreenController._handleAutoPlace";
    pub const TAKE_ALL: &'static str = "ContainerScreenController._handleTakeAll";
    pub const PLACE_ALL: &'static str = "ContainerScreenController._handlePlaceAll";
    pub const DROP_ITEM: &'static str = "ContainerScreenController._handleDropItem";
    pub const DROP_SELECTED: &'static str = "ContainerScreenController._handleDropSelectedItem";
    pub const MANAGER: &'static str = "ContainerScreenController.mContainerManagerController";
    pub const SWAP: &'static str = "ContainerManagerController.handleSwap";
    pub const ENTRIES: [&'static str; 7] = [
        Self::AUTO_PLACE,
        Self::TAKE_ALL,
        Self::PLACE_ALL,
        Self::DROP_ITEM,
        Self::DROP_SELECTED,
        Self::MANAGER,
        Self::SWAP,
    ];

    // the calls of the running build; an error names the first entry that is missing
    pub fn resolve<H: GameHost>(host: &H) -> Result<Self, SdkError> {
        Ok(Self {
            auto_place: host.resolve(Self::AUTO_PLACE)? as usize,
            take_all: host.resolve(Self::TAKE_ALL)? as usize,
            place_all_slot: host.resolve(Self::PLACE_ALL)?,
            drop_item: host.resolve(Self::DROP_ITEM)? as usize,
            drop_selected: host.resolve(Self::DROP_SELECTED)? as usize,
            manager: host.resolve(Self::MANAGER)?,
            swap_slot: host.resolve(Self::SWAP)?,
        })
    }

    // the first entry that does not resolve
    pub fn missing<H: GameHost>(host: &H) -> Option<&'static str> {
        Self::ENTRIES.into_iter().find(|entry| host.resolve(entry).is_err())
    }
}
