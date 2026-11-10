// The input mode of a ScreenView (`ScreenView::mInputMode`, the game's `InputMode` enum). The click router takes a
// pointer press over a mod button only in a pointer mode: in gamepad mode the game sends a press to the focused
// control, not to the control under the pointer (docs/findings/spike-click/report.md section 5)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputMode {
    Mouse,
    Touch,
    GamePad,
    MotionController,
    // 0 (Undefined) or a value the enum does not have
    Unknown,
}

impl InputMode {
    // the raw enum value: 1 Mouse, 2 Touch, 3 GamePad, 4 MotionController (LeviLamina header); any other is Unknown
    pub fn from_value(value: u32) -> Self {
        match value {
            1 => Self::Mouse,
            2 => Self::Touch,
            3 => Self::GamePad,
            4 => Self::MotionController,
            _ => Self::Unknown,
        }
    }

    // whether a press goes to the control under the pointer. Unknown counts as a pointer mode: when the mode cannot
    // be told, a press over a button is taken, so it never reaches a slot under it
    pub fn uses_pointer(self) -> bool {
        !matches!(self, Self::GamePad | Self::MotionController)
    }
}
