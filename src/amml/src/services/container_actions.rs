use amml_api::{Amount, Collection, ContainerScreen, SlotAt};
use hotbar_manager_core::structs::{Collection as CoreCollection, ScreenStep, SlotAddr};

use crate::traits::ScreenActions;

// The container handlers of the game for one open screen, called as the game's own buttons call them: the game
// predicts the move, sends its request and corrects the screen itself. It borrows the `ContainerScreen` of the running
// `ScreenRender`, so it cannot outlive that call
pub struct ContainerActions<'a, 'f> {
    screen: &'a ContainerScreen<'f>,
}

impl<'a, 'f> ContainerActions<'a, 'f> {
    pub fn new(screen: &'a ContainerScreen<'f>) -> Self {
        Self { screen }
    }

    fn slot(addr: SlotAddr) -> SlotAt {
        let (collection, index) = addr.collection();
        let collection = match collection {
            CoreCollection::Container => Collection::Container,
            CoreCollection::Hotbar => Collection::Hotbar,
            CoreCollection::Inventory => Collection::Inventory,
        };
        SlotAt { collection, index }
    }
}

impl ScreenActions for ContainerActions<'_, '_> {
    fn run(&mut self, step: ScreenStep) -> bool {
        match step {
            ScreenStep::AutoPlace(addr) => self.screen.auto_place(Self::slot(addr)).is_ok(),
            ScreenStep::TakeAll(addr) => self.screen.take_all(Self::slot(addr)).is_ok(),
            ScreenStep::PlaceAll(addr) => self.screen.place_all(Self::slot(addr)).is_ok(),
            ScreenStep::Swap(a, b) => self.screen.swap(Self::slot(a), Self::slot(b)).unwrap_or(false),
            ScreenStep::DropSlot(addr) => self.screen.drop_slot(Self::slot(addr), Amount::Whole).is_ok(),
            ScreenStep::DropCursor => self.screen.drop_cursor(Amount::Whole).is_ok(),
        }
    }
}
