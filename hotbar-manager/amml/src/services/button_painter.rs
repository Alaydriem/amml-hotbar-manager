use std::ffi::c_void;

use amml_game::{GameHeap, HashedStringLayout, RawObject};
use amml_msvc::{MsvcHeap, MsvcString};
use hotbar_manager_core::services::ButtonLayout;
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, Glyph, PanelRect, ScreenKind};

use crate::types::{ButtonCalls, ButtonLook};

// Draws the mod buttons as vanilla light glyph buttons (common_buttons.light_glyph_button) through the UI render
// context: per button the state texture inset 1 unit, the focus border at full size in the state's border colour,
// then the glyph centred, 1 unit lower when pressed. One flush per texture (flushImages draws its whole queue with
// the texture of the last queued item). Sources: docs/findings/spike-texture/report.md section 2 and
// spike-align/report.md sections 1, 2 and 4, matched pixel for pixel against the Easy Item Dump pack.
//
// The plan (which texture, which quads, which colour) is pure and tested; the live part loads the textures once,
// then queues and flushes the plan through the context's vtable slots
pub struct ButtonPainter {
    calls: ButtonCalls,
    textures_dir: Option<String>,
    textures: Option<TextureSet>,
    attempted: bool,
    queue_warned: bool,
}

// What one batch draws with
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureKey {
    State(ButtonLook),
    Border,
    Glyph(Glyph),
}

// One drawImage call: GUI units for the position and size, normalized uv
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 2],
    pub uv_size: [f32; 2],
}

// The quads of one texture, then one flush in `color`
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    pub texture: TextureKey,
    pub quads: Vec<Quad>,
    pub color: [f32; 4],
}

// `mce::TexturePtr` (20h): `shared_ptr<BedrockTextureData>`, then `shared_ptr<ResourceLocation>`. drawImage takes
// the first pointer. The mod never releases one: drawImage queues only the pointer, so each must outlive every flush
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct TexturePtrLayout {
    texture: usize,
    _texture_control: usize,
    _location: usize,
    _location_control: usize,
}

// `ResourceLocation` (38h): file system, path, path hash, full hash (docs/findings/spike-texture/report.md section 5)
#[repr(C)]
struct ResourceLocationLayout {
    file_system: u32,
    _pad: u32,
    path: MsvcString,
    path_hash: u64,
    full_hash: u64,
}

const _: () = assert!(size_of::<TexturePtrLayout>() == 0x20);
const _: () = assert!(size_of::<ResourceLocationLayout>() == 0x38);

// the textures of one process: the three states, the border, the glyphs (None where the file did not load) and the
// empty material name, all kept for the life of the process
struct TextureSet {
    states: [TexturePtrLayout; 3],
    border: TexturePtrLayout,
    glyphs: [Option<TexturePtrLayout>; ButtonPainter::GLYPHS.len()],
    // a leaked, writable HashedString: the game's HashedString has a mutable last-match cache
    material: usize,
}

type GetTextureFn = unsafe extern "C" fn(
    *mut c_void,
    *mut TexturePtrLayout,
    *const ResourceLocationLayout,
    u8,
) -> *mut TexturePtrLayout;
type DrawImageFn =
    unsafe extern "C" fn(*mut c_void, usize, *const [f32; 2], *const [f32; 2], *const [f32; 2], *const [f32; 2], u8);
type FlushImagesFn = unsafe extern "C" fn(*mut c_void, *const [f32; 4], f32, *const HashedStringLayout);

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

    // ResourceLocation file systems: 0 the pack stack (the vanilla textures), 2 Raw (an absolute path)
    const FS_USER_PACKAGE: u32 = 0;
    const FS_RAW: u32 = 2;
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    // `textures_dir`: the mod's textures folder, absolute, forward slashes, no trailing slash
    pub fn new(calls: ButtonCalls, textures_dir: Option<String>) -> Self {
        Self { calls, textures_dir, textures: None, attempted: false, queue_warned: false }
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

    // the path hash of a ResourceLocation (the game's string hash, `HashedString::computeHash`) and its full hash,
    // which folds in the file system byte (docs/findings/spike-align/report.md section 2)
    pub fn location_hashes(path: &str, file_system: u32) -> (u64, u64) {
        let path_hash = HashedStringLayout::hash_of(path.as_bytes());
        let full_hash = (u64::from(file_system as u8) ^ Self::FNV_OFFSET).wrapping_mul(Self::FNV_PRIME) ^ path_hash;
        (path_hash, full_hash)
    }

    /// the GUI scale of the context (screen pixels per GUI unit); None when it cannot be read
    ///
    /// # Safety
    /// `ctx` must be null or the live `MinecraftUIRenderContext` of the current render call, of the build the calls
    /// were resolved for
    #[allow(unsafe_code)]
    pub unsafe fn gui_scale(&self, ctx: *mut c_void) -> Option<f32> {
        if ctx.is_null() || self.calls.get_gui_scale == 0 {
            return None;
        }
        // SAFETY: the context holds its ScreenContext* at the entry's offset
        let screen = unsafe { RawObject::pointer(ctx, self.calls.screen_context) }?;
        // SAFETY: the ScreenContext holds the raw GuiData* at the entry's offset
        let gui = unsafe { RawObject::pointer(screen, self.calls.gui_data) }?;
        // SAFETY: the entry is the address of `float GuiData::getGuiScale() const`: this in rcx, the scale in xmm0
        let call: unsafe extern "C" fn(*mut c_void) -> f32 =
            unsafe { std::mem::transmute(self.calls.get_gui_scale as *const c_void) };
        // SAFETY: the GuiData is live
        let scale = unsafe { call(gui) };
        (scale.is_finite() && scale > 0.0).then_some(scale)
    }

    // whether a context's image queue (begin and end of its vector) holds items: flushImages draws its whole queue
    // with the texture of the last queued item, so the buttons' first flush would draw such items with a button
    // texture. The spike skipped the frame then (docs/findings/spike-texture/report.md section 5)
    pub fn queue_holds_items(begin: usize, end: usize) -> bool {
        begin != end
    }

    /// whether the game left images queued in `ctx`, by the optional `MinecraftUIRenderContext.mQueuedImages` entry;
    /// false without the entry (the buttons draw as before). The first busy frame logs one warning
    ///
    /// # Safety
    /// `ctx` must be null or the live `MinecraftUIRenderContext` of the current render call, of the build the calls
    /// were resolved for
    #[allow(unsafe_code)]
    pub unsafe fn queue_busy(&mut self, ctx: *mut c_void) -> bool {
        let Some(offset) = self.calls.queued_images else { return false };
        if ctx.is_null() {
            return false;
        }
        // SAFETY: a live context holds the begin and end pointers of its image queue at the entry's offset
        let begin = unsafe { RawObject::read::<usize>(ctx, offset) };
        // SAFETY: as above
        let end = unsafe { RawObject::read::<usize>(ctx, offset.saturating_add(8)) };
        let busy = matches!((begin, end), (Some(begin), Some(end)) if Self::queue_holds_items(begin, end));
        if busy && !self.queue_warned {
            self.queue_warned = true;
            Self::warn("the game left images queued after the screen render; the buttons skip such frames");
        }
        busy
    }

    // whether the textures loaded and the glyph of `glyph` with them
    pub fn can_draw(&self, glyph: Glyph) -> bool {
        self.textures.as_ref().is_some_and(|set| set.glyph(glyph).is_some())
    }

    /// draws the buttons of `kind` on `panel` and gives back the ones it drew (a button whose glyph did not load is
    /// not drawn, and must not take clicks). Nothing is drawn without a GUI scale or without the textures
    ///
    /// # Safety
    /// `ctx` must be the live `MinecraftUIRenderContext` of the current `ScreenView::render` call, after the original,
    /// on the game thread, of the build the calls were resolved for
    #[allow(unsafe_code)]
    pub unsafe fn paint(
        &mut self,
        ctx: *mut c_void,
        kind: ScreenKind,
        panel: PanelRect,
        scale: f32,
        hovered: Option<ButtonAction>,
        pressed: Option<ButtonAction>,
    ) -> Vec<ButtonSpec> {
        if ctx.is_null() || !(scale.is_finite() && scale > 0.0) {
            return Vec::new();
        }
        // SAFETY: forwarded to the caller
        unsafe { self.ensure_textures(ctx) };
        let Some(set) = self.textures.as_ref() else { return Vec::new() };
        // SAFETY: the live context, whose vtable holds drawImage and flushImages at the entries' slots
        let draw = unsafe { RawObject::slot(ctx, self.calls.draw_image_slot) };
        // SAFETY: as above
        let flush = unsafe { RawObject::slot(ctx, self.calls.flush_images_slot) };
        let (Some(draw), Some(flush)) = (draw, flush) else { return Vec::new() };
        // SAFETY: the slots hold `void drawImage(BedrockTextureData const&, vec2 const&, vec2 const&, vec2 const&,
        // vec2 const&, bool)` and `void flushImages(mce::Color const&, float, HashedString const&)`
        let draw = unsafe { std::mem::transmute::<*const c_void, DrawImageFn>(draw) };
        // SAFETY: as above
        let flush = unsafe { std::mem::transmute::<*const c_void, FlushImagesFn>(flush) };
        let material = set.material as *const HashedStringLayout;
        let mut drawn = Vec::new();
        for spec in ButtonLayout::for_screen(kind) {
            if set.glyph(spec.glyph).is_none() {
                continue;
            }
            let look = Self::look_of(spec.action, hovered, pressed);
            for batch in Self::plan(&spec, panel.resolve(&spec), look) {
                let Some(texture) = set.texture(batch.texture) else { continue };
                for quad in &batch.quads {
                    // SAFETY: the context is live; the texture data is held by a TexturePtr the mod never releases,
                    // and the vectors live across the call (drawImage copies them into its queue item)
                    unsafe {
                        draw(ctx, texture, &quad.position, &quad.size, &quad.uv, &quad.uv_size, 0);
                    }
                }
                // SAFETY: the context is live; the material is a leaked, writable HashedString
                unsafe { flush(ctx, &batch.color, 1.0, material) };
            }
            drawn.push(spec);
        }
        drawn
    }

    // loads every texture on the first paint; a later paint does not try again, so a failed load costs one warning
    // and no leak per frame
    #[allow(unsafe_code)]
    unsafe fn ensure_textures(&mut self, ctx: *mut c_void) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        // SAFETY: forwarded to the caller
        self.textures = unsafe { self.load(ctx) };
        if self.textures.is_none() {
            Self::warn("the vanilla button textures did not load; the buttons are off");
        }
    }

    #[allow(unsafe_code)]
    unsafe fn load(&self, ctx: *mut c_void) -> Option<TextureSet> {
        let heap = GameHeap::msvc()?;
        let vanilla = |path: &str| {
            // SAFETY: forwarded to the caller of `load`
            unsafe { self.texture(ctx, &heap, path, Self::FS_USER_PACKAGE) }
        };
        let states = [vanilla(Self::STATE_PATHS[0])?, vanilla(Self::STATE_PATHS[1])?, vanilla(Self::STATE_PATHS[2])?];
        let border = vanilla(Self::BORDER_PATH)?;
        let glyphs = Self::GLYPHS.map(|glyph| {
            let path = format!("{}/{}", self.textures_dir.as_deref()?, Self::glyph_file(glyph));
            // SAFETY: forwarded to the caller of `load`
            let texture = unsafe { self.texture(ctx, &heap, &path, Self::FS_RAW) };
            if texture.is_none() {
                Self::warn(&format!("the button glyph {path}.png did not load; its button is off"));
            }
            texture
        });
        // SAFETY: the heap is the game's; an empty string has no heap block
        let name = unsafe { MsvcString::new_in(&heap, "") }.ok()?;
        let material = Box::leak(Box::new(HashedStringLayout::new(name))) as *mut HashedStringLayout as usize;
        Some(TextureSet { states, border, glyphs, material })
    }

    // one texture through getTexture; None when the game gives an empty TexturePtr
    #[allow(unsafe_code)]
    unsafe fn texture(
        &self,
        ctx: *mut c_void,
        heap: &MsvcHeap,
        path: &str,
        file_system: u32,
    ) -> Option<TexturePtrLayout> {
        let location = Self::location(heap, path, file_system)?;
        // SAFETY: the live context, whose vtable holds getTexture at the entry's slot
        let function = unsafe { RawObject::slot(ctx, self.calls.get_texture_slot) }?;
        // SAFETY: the slot holds `mce::TexturePtr getTexture(ResourceLocation const&, bool)`: this in rcx, the hidden
        // return slot in rdx, the location in r8, forceReload in r9b
        let call: GetTextureFn = unsafe { std::mem::transmute(function) };
        let mut texture = TexturePtrLayout::default();
        // SAFETY: the context is live, the return slot is 20h writable bytes, the location is live
        unsafe { call(ctx, &mut texture, location, 0) };
        (texture.texture != 0).then_some(texture)
    }

    // a ResourceLocation, leaked on purpose: the game may keep a reference to it (docs/findings/spike-texture
    // section 5). There are at most 11 in a process, made once
    fn location(heap: &MsvcHeap, path: &str, file_system: u32) -> Option<&'static ResourceLocationLayout> {
        // SAFETY: the heap is the game's, so the game may free the block as its own
        #[allow(unsafe_code)]
        let text = unsafe { MsvcString::new_in(heap, path) }.ok()?;
        let (path_hash, full_hash) = Self::location_hashes(path, file_system);
        Some(Box::leak(Box::new(ResourceLocationLayout { file_system, _pad: 0, path: text, path_hash, full_hash })))
    }

    fn warn(message: &str) {
        if let Some(runtime) = amml_sdk::ModRuntime::get() {
            runtime.logger().warn(message);
        }
    }
}

impl TextureSet {
    fn glyph(&self, glyph: Glyph) -> Option<&TexturePtrLayout> {
        let index = ButtonPainter::GLYPHS.iter().position(|known| *known == glyph)?;
        self.glyphs.get(index)?.as_ref()
    }

    // the BedrockTextureData pointer of a batch's texture
    fn texture(&self, key: TextureKey) -> Option<usize> {
        let texture = match key {
            TextureKey::State(ButtonLook::Default) => &self.states[0],
            TextureKey::State(ButtonLook::Hover) => &self.states[1],
            TextureKey::State(ButtonLook::Pressed) => &self.states[2],
            TextureKey::Border => &self.border,
            TextureKey::Glyph(glyph) => self.glyph(glyph)?,
        };
        Some(texture.texture)
    }
}
