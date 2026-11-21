use amml_api::{Collection, ContainerScreen};
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ScreenKind, SlotItem};

use crate::services::SlotConverter;
use crate::traits::ScreenSource;

// What the actions read of the open container screen, one fresh read per call: its slots and the cursor stack. It
// borrows the `ContainerScreen` of the running `ScreenRender`, so it cannot outlive that call
pub struct ContainerSource<'a, 'f> {
    screen: &'a ContainerScreen<'f>,
    kind: ScreenKind,
}

impl<'a, 'f> ContainerSource<'a, 'f> {
    pub fn new(screen: &'a ContainerScreen<'f>, kind: ScreenKind) -> Self {
        Self { screen, kind }
    }
}

impl ScreenSource for ContainerSource<'_, '_> {
    fn kind(&self) -> ScreenKind {
        self.kind
    }

    fn read(&self) -> Option<SlotModel> {
        let container = self.screen.slots(Collection::Container)?;
        let hotbar = self.screen.slots(Collection::Hotbar)?;
        let inventory = self.screen.slots(Collection::Inventory)?;
        SlotConverter::model(self.kind, &container, &hotbar, &inventory)
    }

    fn cursor(&self) -> Option<Option<SlotItem>> {
        let cursor = self.screen.cursor()?;
        SlotConverter::slot(cursor.as_ref())
    }
}
