// The UI profile of the game: the classic panels (the container screens this mod draws buttons on) or the pocket
// screens, which have no classic panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiProfile {
    Classic,
    Pocket,
}

impl UiProfile {
    // the value `IOptionsReader::getUIProfile` returns: 0 Classic, 1 Pocket (docs/findings/spike-panel/report.md
    // section 4). Any other value is Classic: it is the game's default profile, and in a profile that is not Classic
    // the panel lookup finds no `root_panel` and nothing is drawn, so a wrong guess costs no wrong button
    pub fn from_value(value: i32) -> Self {
        match value {
            1 => Self::Pocket,
            _ => Self::Classic,
        }
    }
}
