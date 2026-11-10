use crate::structs::{ButtonAction, ButtonSpec, Glyph, ScreenKind};

/// Where the buttons sit on each screen.
pub struct ButtonLayout;

const SIDE_SIZE: (f32, f32) = (15.0, 30.0);
const SIDE_GLYPH: (f32, f32) = (8.0, 24.0);
const TOP_SIZE: (f32, f32) = (30.0, 15.0);
const TOP_GLYPH: (f32, f32) = (24.0, 8.0);

impl ButtonLayout {
    pub fn for_screen(kind: ScreenKind) -> Vec<ButtonSpec> {
        match kind {
            ScreenKind::SmallChest
            | ScreenKind::LargeChest
            | ScreenKind::Barrel
            | ScreenKind::ShulkerBox
            | ScreenKind::EnderChest => vec![
                Self::side(ButtonAction::DumpFromInventory, Glyph::DumpFromInventory, (-30.0, 0.0)),
                Self::side(ButtonAction::DumpFromContainer, Glyph::DumpFromContainer, (-15.0, 0.0)),
                Self::side(ButtonAction::DumpFromInventoryFiltered, Glyph::DumpFromInventoryFiltered, (-30.0, 30.0)),
                Self::side(ButtonAction::DumpFromContainerFiltered, Glyph::DumpFromContainerFiltered, (-15.0, 30.0)),
                Self::sort(),
            ],
            ScreenKind::Hopper | ScreenKind::Dispenser | ScreenKind::Dropper => vec![
                Self::side(ButtonAction::DumpFromContainerFiltered, Glyph::DumpFromContainerFiltered, (-15.0, 7.0)),
                Self::side(ButtonAction::DumpFromInventoryFiltered, Glyph::DumpFromInventoryFiltered, (-30.0, 7.0)),
                Self::sort(),
            ],
            ScreenKind::PlayerInventory => vec![
                Self::top(ButtonAction::DropAll, Glyph::Drop, (0.0, -30.0)),
                Self::top(ButtonAction::DropFiltered, Glyph::DropFiltered, (0.0, -15.0)),
            ],
            ScreenKind::Other => Vec::new(),
        }
    }

    fn side(action: ButtonAction, glyph: Glyph, offset: (f32, f32)) -> ButtonSpec {
        ButtonSpec { action, offset, size: SIDE_SIZE, glyph, glyph_size: SIDE_GLYPH }
    }

    fn top(action: ButtonAction, glyph: Glyph, offset: (f32, f32)) -> ButtonSpec {
        ButtonSpec { action, offset, size: TOP_SIZE, glyph, glyph_size: TOP_GLYPH }
    }

    // directly left of the close button, on its centre line
    fn sort() -> ButtonSpec {
        ButtonSpec {
            action: ButtonAction::Sort,
            offset: (142.0, 7.0),
            size: (11.0, 11.0),
            glyph: Glyph::Sort,
            glyph_size: (7.0, 7.0),
        }
    }
}
