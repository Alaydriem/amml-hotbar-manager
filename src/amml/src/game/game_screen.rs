use std::ffi::c_void;

use amml_game::{GameEntries, ScreenView};
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ScreenKind, SlotItem};

use crate::services::SlotReader;
use crate::traits::ScreenSource;

// The open screen as the game shows it: the slots and the cursor stack through the view's container controller and the
// slot reader. Built for one frame, from the live view of that moment; it keeps nothing. Game thread only
pub struct GameScreen<'a> {
    entries: &'a GameEntries,
    reader: &'a SlotReader,
    view: *mut c_void,
    kind: ScreenKind,
}

impl<'a> GameScreen<'a> {
    /// the screen of `view`, of kind `kind`
    ///
    /// # Safety
    /// `view` must be the live `ScreenView` of the build `entries` were resolved for, on the game thread, and must
    /// stay live while the value is used
    #[allow(unsafe_code)]
    pub unsafe fn new(entries: &'a GameEntries, reader: &'a SlotReader, view: usize, kind: ScreenKind) -> Self {
        Self { entries, reader, view: view as *mut c_void, kind }
    }

    /// the address of the view's controller, the identity of the screen (both views of a screen give the same one)
    #[allow(unsafe_code)]
    pub fn controller_address(&self) -> Option<usize> {
        // SAFETY: `new` guarantees a live view of the build the entries belong to
        let view = unsafe { ScreenView::from_raw(self.view, self.entries) }?;
        view.controller().map(|controller| controller.address())
    }

    /// The `ContainerScreenController` of the view, the object the container handlers are called on: the pointer
    /// `getVisualItemStackImpl` is called on for the slot reads. The SDK's `container()` keeps the address of the
    /// view's `ScreenController` (no cast adjusts it), so this is the same address as `controller_address`. None when
    /// the view has no controller or it is not a container screen. Valid only inside the view's render call
    #[allow(unsafe_code)]
    pub fn container_controller(&self) -> Option<*mut c_void> {
        // SAFETY: `new` guarantees a live view of the build the entries belong to
        let view = unsafe { ScreenView::from_raw(self.view, self.entries) }?;
        Some(view.controller()?.container()?.as_ptr())
    }
}

impl ScreenSource for GameScreen<'_> {
    fn kind(&self) -> ScreenKind {
        self.kind
    }

    #[allow(unsafe_code)]
    fn read(&self) -> Option<SlotModel> {
        // SAFETY: `new` guarantees a live view of the build the entries belong to
        let view = unsafe { ScreenView::from_raw(self.view, self.entries) }?;
        let controller = view.controller()?.container()?;
        self.reader.read(&controller, self.kind)
    }

    #[allow(unsafe_code)]
    fn cursor(&self) -> Option<Option<SlotItem>> {
        // SAFETY: `new` guarantees a live view of the build the entries belong to
        let view = unsafe { ScreenView::from_raw(self.view, self.entries) }?;
        let controller = view.controller()?.container()?;
        self.reader.read_cursor(&controller)
    }
}
