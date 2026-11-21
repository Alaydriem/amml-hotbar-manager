use amml_api::{ApiError, Color, Logger, Rect, TextureId, TextureReload, TextureSource, Textures, Tint, UiDraw, Uv};
use hotbar_manager_core::services::ButtonLayout;
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, Glyph, PanelRect, ScreenKind};

use crate::types::ButtonLook;

// Draws the mod buttons as vanilla light glyph buttons (common_buttons.light_glyph_button) through the loader's UI draw:
// per button the state texture inset 1 unit, the focus border at full size in the state's border colour, then the glyph
// centred, 1 unit lower when pressed. Sources: docs/findings/spike-texture/report.md section 2 and
// spike-align/report.md sections 1, 2 and 4, matched pixel for pixel against the Easy Item Dump pack.
//
// The plan (which texture, which quads, which colour) is pure and tested; the live part asks the loader for the
// textures once and draws the plan with `UiDraw::image`. The loader binds a texture again when the game drops its own
// (a world or dimension change), so no button draws pink; a texture of a world that was left is `Stale`, and the next
// paint asks for the set again
pub struct ButtonPainter {
    textures: Textures,
    log: Logger,
    set: Option<TextureSet>,
    attempted: bool,
    // paints before every texture was ready, and whether the first paint with all of them ready was logged
    loading_paints: u32,
    ready_logged: bool,
}

// What one batch draws with
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureKey {
    State(ButtonLook),
    Border,
    Glyph(Glyph),
}

// One image of the draw: GUI units for the position and size, normalized uv
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 2],
    pub uv_size: [f32; 2],
}

// The quads of one texture, all in `color`
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub texture: TextureKey,
    pub quads: Vec<Quad>,
    pub color: [f32; 4],
}

// the textures of the buttons: the three states, the border and the glyphs (None where the file did not load)
struct TextureSet {
    states: [TextureId; 3],
    border: TextureId,
    glyphs: [Option<TextureId>; ButtonPainter::GLYPHS.len()],
}

impl ButtonPainter {
    pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    // _global_variables.json: $light_border_default_color and $light_glyph_default_color
    pub const BORDER_DEFAULT: [f32; 4] = [0.075, 0.075, 0.075, 1.0];
    pub const GLYPH_DEFAULT: [f32; 4] = [0.3, 0.3, 0.3, 1.0];
    // nine-slice: 1 texel = 1 GUI unit; corners 1 texel; the state pngs are 4x4, the border png 5x5
    pub const SLICE: f32 = 1.0;
    pub const STATE_BASE: f32 = 4.0;
    pub const BORDER_BASE: f32 = 5.0;
    // $button_image_size = 100% - 2px: the state image is inset 1 unit on each side
    pub const STATE_INSET: f32 = 1.0;
    // $button_pressed_offset
    pub const PRESSED_OFFSET: f32 = 1.0;

    pub const STATE_PATHS: [&'static str; 3] = [
        "textures/ui/button_borderless_light",
        "textures/ui/button_borderless_lighthover",
        "textures/ui/button_borderless_lightpressed",
    ];
    pub const BORDER_PATH: &'static str = "textures/ui/focus_border_white";
    pub const GLYPHS: [Glyph; 7] = [
        Glyph::Sort,
        Glyph::DumpFromInventory,
        Glyph::DumpFromContainer,
        Glyph::DumpFromInventoryFiltered,
        Glyph::DumpFromContainerFiltered,
        Glyph::Drop,
        Glyph::DropFiltered,
    ];

    pub fn new(textures: Textures, log: Logger) -> Self {
        Self { textures, log, set: None, attempted: false, loading_paints: 0, ready_logged: false }
    }

    // the file of each glyph in the mod's textures folder, without the extension
    pub fn glyph_file(glyph: Glyph) -> &'static str {
        match glyph {
            Glyph::Sort => "sort",
            Glyph::DumpFromInventory => "ez_dump_from_inventory",
            Glyph::DumpFromContainer => "ez_dump_from_chest",
            Glyph::DumpFromInventoryFiltered => "ez_dump_from_inventory_filtered",
            Glyph::DumpFromContainerFiltered => "ez_dump_from_chest_filtered",
            Glyph::Drop => "ez_drop",
            Glyph::DropFiltered => "ez_drop_filtered",
        }
    }

    // the glyph colour in the default look: the Easy Item Dump glyphs are full colour and the pack tints them white
    // ($default_glyph_color = $light_glyph_hover_color); Sort is a vanilla glyph, 0.3 grey
    pub fn default_glyph_color(glyph: Glyph) -> [f32; 4] {
        match glyph {
            Glyph::Sort => Self::GLYPH_DEFAULT,
            _ => Self::WHITE,
        }
    }

    // the look of `action`: Pressed when it is the pressed button, else Hover when it is under the pointer
    pub fn look_of(action: ButtonAction, hovered: Option<ButtonAction>, pressed: Option<ButtonAction>) -> ButtonLook {
        if pressed == Some(action) {
            ButtonLook::Pressed
        } else if hovered == Some(action) {
            ButtonLook::Hover
        } else {
            ButtonLook::Default
        }
    }

    // the nine pieces of `rect` for a `base`-texel square texture with `slice`-texel corners; pieces of no size are
    // left out
    pub fn nine_slice(rect: PanelRect, slice: f32, base: f32) -> Vec<Quad> {
        let xs = [rect.x, rect.x + slice, rect.x + rect.w - slice];
        let ws = [slice, rect.w - 2.0 * slice, slice];
        let ys = [rect.y, rect.y + slice, rect.y + rect.h - slice];
        let hs = [slice, rect.h - 2.0 * slice, slice];
        let us = [0.0, slice / base, (base - slice) / base];
        let uws = [slice / base, (base - 2.0 * slice) / base, slice / base];
        let mut quads = Vec::with_capacity(9);
        for row in 0..3 {
            for column in 0..3 {
                if ws[column] <= 0.0 || hs[row] <= 0.0 {
                    continue;
                }
                quads.push(Quad {
                    position: [xs[column], ys[row]],
                    size: [ws[column], hs[row]],
                    uv: [us[column], us[row]],
                    uv_size: [uws[column], uws[row]],
                });
            }
        }
        quads
    }

    // the three batches of one button at `rect` (screen space): state, border, glyph
    pub fn plan(spec: &ButtonSpec, rect: PanelRect, look: ButtonLook) -> [Batch; 3] {
        let inset = Self::STATE_INSET;
        let state_rect =
            PanelRect { x: rect.x + inset, y: rect.y + inset, w: rect.w - 2.0 * inset, h: rect.h - 2.0 * inset };
        let (border, glyph_color, offset) = match look {
            ButtonLook::Default => (Self::BORDER_DEFAULT, Self::default_glyph_color(spec.glyph), 0.0),
            ButtonLook::Hover => (Self::WHITE, Self::WHITE, 0.0),
            ButtonLook::Pressed => (Self::WHITE, Self::WHITE, Self::PRESSED_OFFSET),
        };
        let (gw, gh) = spec.glyph_size;
        let glyph = Quad {
            position: [rect.x + (rect.w - gw) / 2.0, rect.y + (rect.h - gh) / 2.0 + offset],
            size: [gw, gh],
            uv: [0.0, 0.0],
            uv_size: [1.0, 1.0],
        };
        [
            Batch {
                texture: TextureKey::State(look),
                quads: Self::nine_slice(state_rect, Self::SLICE, Self::STATE_BASE),
                color: Self::WHITE,
            },
            Batch {
                texture: TextureKey::Border,
                quads: Self::nine_slice(rect, Self::SLICE, Self::BORDER_BASE),
                color: border,
            },
            Batch { texture: TextureKey::Glyph(spec.glyph), quads: vec![glyph], color: glyph_color },
        ]
    }

    // the loader bound or freed the textures. After a device reset every id is stale: the set is asked for again at the
    // next paint. After a resource pack change the ids stay valid and the loader binds the files again. Either way the
    // ready state is read again, so `textures ready` is logged when the set is ready after the reload. Gives the number
    // of textures that were held
    pub fn textures_reloaded(&mut self, reason: TextureReload) -> usize {
        let held = self.set.as_ref().map_or(0, |set| set.ids().len());
        if reason == TextureReload::DeviceReset {
            self.set = None;
            self.attempted = false;
        }
        self.ready_logged = false;
        self.loading_paints = 0;
        held
    }

    // whether the textures loaded and the glyph of `glyph` with them
    pub fn can_draw(&self, glyph: Glyph) -> bool {
        self.set.as_ref().is_some_and(|set| set.glyph(glyph).is_some())
    }

    // draws the buttons of `kind` on `panel` and gives back the ones it drew (a button whose glyph did not load is not
    // drawn, and must not take clicks). Nothing is drawn while the textures cannot be asked for
    pub fn paint(
        &mut self,
        draw: &UiDraw<'_>,
        kind: ScreenKind,
        panel: PanelRect,
        hovered: Option<ButtonAction>,
        pressed: Option<ButtonAction>,
    ) -> Vec<ButtonSpec> {
        self.ensure_set();
        let Some(set) = self.set.as_ref() else { return Vec::new() };
        let mut drawn = Vec::new();
        let mut stale = false;
        'buttons: for spec in ButtonLayout::for_screen(kind) {
            if set.glyph(spec.glyph).is_none() {
                continue;
            }
            let look = Self::look_of(spec.action, hovered, pressed);
            for batch in Self::plan(&spec, panel.resolve(&spec), look) {
                let Some(texture) = set.texture(batch.texture) else { continue };
                let tint = Tint::new(Color { r: batch.color[0], g: batch.color[1], b: batch.color[2] }, batch.color[3]);
                for quad in &batch.quads {
                    // a texture that is not ready is skipped by the loader and the call gives `Ok`
                    if let Err(ApiError::Stale) = draw.image(texture, Self::rect(quad), Self::uv(quad), tint) {
                        stale = true;
                        break 'buttons;
                    }
                }
            }
            drawn.push(spec);
        }
        if stale {
            // the textures of a world that was left: ask for the set again at the next paint
            self.set = None;
            self.attempted = false;
            self.log.info("button textures were stale; asking for them again", &[]);
            return drawn;
        }
        self.note_ready();
        drawn
    }

    fn rect(quad: &Quad) -> Rect {
        Rect { x: quad.position[0], y: quad.position[1], width: quad.size[0], height: quad.size[1] }
    }

    fn uv(quad: &Quad) -> Uv {
        Uv { u0: quad.uv[0], v0: quad.uv[1], u1: quad.uv[0] + quad.uv_size[0], v1: quad.uv[1] + quad.uv_size[1] }
    }

    // asks for every texture on the first paint; a later paint does not try again unless the set went stale, so a
    // failed load costs one warning and no call per frame
    fn ensure_set(&mut self) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        self.set = self.load();
        if self.set.is_none() {
            self.log.warn("the vanilla button textures did not load; the buttons are off", &[]);
        }
    }

    fn load(&self) -> Option<TextureSet> {
        let vanilla = |path: &str| self.textures.load(TextureSource::Vanilla(path)).ok();
        let states = [vanilla(Self::STATE_PATHS[0])?, vanilla(Self::STATE_PATHS[1])?, vanilla(Self::STATE_PATHS[2])?];
        let border = vanilla(Self::BORDER_PATH)?;
        let glyphs = Self::GLYPHS.map(|glyph| {
            let path = format!("textures/{}.png", Self::glyph_file(glyph));
            let texture = self.textures.load(TextureSource::ModFile(&path)).ok();
            if texture.is_none() {
                self.log.warn("a button glyph did not load; its button is off", &[("path", &path)]);
            }
            texture
        });
        Some(TextureSet { states, border, glyphs })
    }

    // logs once, at the first paint with every texture ready, how many there are and how many paints it took
    fn note_ready(&mut self) {
        if self.ready_logged {
            return;
        }
        let Some(set) = self.set.as_ref() else { return };
        let ids = set.ids();
        if ids.iter().all(|id| self.textures.ready(*id)) {
            self.ready_logged = true;
            self.log.info(
                "textures ready",
                &[("count", &ids.len().to_string()), ("paints", &self.loading_paints.to_string())],
            );
        } else {
            self.loading_paints = self.loading_paints.saturating_add(1);
        }
    }
}

impl TextureSet {
    fn glyph(&self, glyph: Glyph) -> Option<TextureId> {
        let index = ButtonPainter::GLYPHS.iter().position(|known| *known == glyph)?;
        *self.glyphs.get(index)?
    }

    // the texture of a batch
    fn texture(&self, key: TextureKey) -> Option<TextureId> {
        match key {
            TextureKey::State(ButtonLook::Default) => Some(self.states[0]),
            TextureKey::State(ButtonLook::Hover) => Some(self.states[1]),
            TextureKey::State(ButtonLook::Pressed) => Some(self.states[2]),
            TextureKey::Border => Some(self.border),
            TextureKey::Glyph(glyph) => self.glyph(glyph),
        }
    }

    // every texture that loaded
    fn ids(&self) -> Vec<TextureId> {
        self.states.iter().copied().chain([self.border]).chain(self.glyphs.iter().flatten().copied()).collect()
    }
}
