/// What a button does when the player clicks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonAction {
    Sort,
    DumpFromInventory,
    DumpFromContainer,
    DumpFromInventoryFiltered,
    DumpFromContainerFiltered,
    DropAll,
    DropFiltered,
}

/// The picture on a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Glyph {
    Sort,
    DumpFromInventory,
    DumpFromContainer,
    DumpFromInventoryFiltered,
    DumpFromContainerFiltered,
    Drop,
    DropFiltered,
}

/// One button. All values are JSON UI units; `offset` is relative to the top-left of the panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonSpec {
    pub action: ButtonAction,
    pub offset: (f32, f32),
    pub size: (f32, f32),
    pub glyph: Glyph,
    pub glyph_size: (f32, f32),
}

/// A rectangle in JSON UI units, in screen space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl PanelRect {
    /// The rectangle of a button on this panel.
    pub fn resolve(&self, spec: &ButtonSpec) -> PanelRect {
        PanelRect { x: self.x + spec.offset.0, y: self.y + spec.offset.1, w: spec.size.0, h: spec.size.1 }
    }
}
