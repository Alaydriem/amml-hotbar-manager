use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ScreenKind, SlotItem};

// what the button actions read of the open screen: its kind, the slots the client shows now and the cursor stack. The game gives it through `GameScreen`; the tests give a fake. A read is made fresh each time: no model is kept
pub trait ScreenSource {
    fn kind(&self) -> ScreenKind;

    // the slots of the screen; None when any of them cannot be read
    fn read(&self) -> Option<SlotModel>;

    // the stack on the cursor: Some(None) when it is empty, None when it cannot be read
    fn cursor(&self) -> Option<Option<SlotItem>>;
}
