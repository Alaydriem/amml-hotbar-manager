use amml_sdk::SdkError;

use crate::traits::GameHost;

// The resolved game code and offsets the buttons use beyond `ScreenCalls`: the draw calls of the UI render context
// and the GUI scale (docs/findings/spike-texture/report.md section 1, spike-panel section 1), the panel walk through
// the visual tree, and the pointer of the view (spike-click section 2). Every one has recipes in the hotbar worktree.
// With any of them missing the buttons are off. `ScreenView.mInputMode` is optional and has no recipe yet: without
// it the input mode cannot be told, and a pointer press over a button is taken in every mode.
// `MinecraftUIRenderContext.mQueuedImages` is optional and has no recipe yet: without it the buttons draw without
// checking that the game left no image queued
#[derive(Clone, Copy, Debug)]
pub struct ButtonCalls {
    // offset of `MinecraftUIRenderContext::mScreenContext` (a `ScreenContext*`)
    pub screen_context: u64,
    // vtable slot of `void drawImage(BedrockTextureData const&, vec2 const&, vec2 const&, vec2 const&, vec2 const&,
    // bool)`
    pub draw_image_slot: u64,
    // vtable slot of `void flushImages(mce::Color const&, float, HashedString const&)`
    pub flush_images_slot: u64,
    // vtable slot of `mce::TexturePtr getTexture(ResourceLocation const&, bool)`
    pub get_texture_slot: u64,
    // offset of the raw `GuiData*` in the `ScreenContext`
    pub gui_data: u64,
    // the address of `float GuiData::getGuiScale() const`
    pub get_gui_scale: usize,
    // offset of `UIControl::mChildren` (a vector of 10h-byte `shared_ptr<UIControl>`)
    pub children: u64,
    // offset of `UIControl::mCachedPosition` (vec2, absolute, GUI units)
    pub cached_position: u64,
    // offset of the flag byte that holds the cached-position dirty bit, and the bit's mask
    pub cached_position_dirty: u64,
    pub cached_position_dirty_mask: u64,
    // offset of the flag byte that holds the visible-in-tree bit, and the bit's mask
    pub visible_in_tree: u64,
    pub visible_in_tree_mask: u64,
    // offset of `UIControl::mSize` (vec2, GUI units)
    pub size: u64,
    // the address of `void UIControl::_setCachedPosition() const`
    pub set_cached_position: usize,
    // offset of `ScreenView::mPointerButtonIds` (`std::array<uint, 2>`)
    pub pointer_button_ids: u64,
    // offset of `ScreenView::mPointerLocationPrevious` (vec2, GUI units)
    pub pointer_location: u64,
    // offset of `ScreenView::mInputMode` (the `InputMode` enum, 4 bytes); optional, no recipe yet
    pub input_mode: Option<u64>,
    // offset of the context's queued image items (a `std::vector`: begin, then end 8 bytes later); optional, no
    // recipe yet
    pub queued_images: Option<u64>,
}

impl ButtonCalls {
    pub const SCREEN_CONTEXT: &'static str = "MinecraftUIRenderContext.mScreenContext";
    pub const DRAW_IMAGE: &'static str = "MinecraftUIRenderContext.drawImage";
    pub const FLUSH_IMAGES: &'static str = "MinecraftUIRenderContext.flushImages";
    pub const GET_TEXTURE: &'static str = "MinecraftUIRenderContext.getTexture";
    pub const GUI_DATA: &'static str = "ScreenContext.guiData";
    pub const GET_GUI_SCALE: &'static str = "GuiData.getGuiScale";
    pub const CHILDREN: &'static str = "UIControl.mChildren";
    pub const CACHED_POSITION: &'static str = "UIControl.mCachedPosition";
    pub const CACHED_POSITION_DIRTY: &'static str = "UIControl.mCachedPositionDirty";
    pub const CACHED_POSITION_DIRTY_MASK: &'static str = "UIControl.mCachedPositionDirtyMask";
    pub const VISIBLE_IN_TREE: &'static str = "UIControl.mIsVisibleInTree";
    pub const VISIBLE_IN_TREE_MASK: &'static str = "UIControl.mIsVisibleInTreeMask";
    pub const SIZE: &'static str = "UIControl.mSize";
    pub const SET_CACHED_POSITION: &'static str = "UIControl._setCachedPosition";
    pub const POINTER_BUTTON_IDS: &'static str = "ScreenView.mPointerButtonIds";
    pub const POINTER_LOCATION: &'static str = "ScreenView.mPointerLocationPrevious";
    // optional, no recipe yet (the raw value is ScreenView +200h on 1.26.62.01, read by handleButtonEvent)
    pub const INPUT_MODE: &'static str = "ScreenView.mInputMode";
    // optional, no recipe yet (the raw value is +98h on 1.26.62.01: flushImages reads the vector at [rcx+98h] and
    // [rcx+A0h])
    pub const QUEUED_IMAGES: &'static str = "MinecraftUIRenderContext.mQueuedImages";
    pub const ENTRIES: [&'static str; 18] = [
        Self::SCREEN_CONTEXT,
        Self::DRAW_IMAGE,
        Self::FLUSH_IMAGES,
        Self::GET_TEXTURE,
        Self::GUI_DATA,
        Self::GET_GUI_SCALE,
        Self::CHILDREN,
        Self::CACHED_POSITION,
        Self::CACHED_POSITION_DIRTY,
        Self::CACHED_POSITION_DIRTY_MASK,
        Self::VISIBLE_IN_TREE,
        Self::VISIBLE_IN_TREE_MASK,
        Self::SIZE,
        Self::SET_CACHED_POSITION,
        Self::POINTER_BUTTON_IDS,
        Self::POINTER_LOCATION,
        Self::INPUT_MODE,
        Self::QUEUED_IMAGES,
    ];

    // the calls of the running build; an error names the first required entry that is missing
    pub fn resolve<H: GameHost>(host: &H) -> Result<Self, SdkError> {
        Ok(Self {
            screen_context: host.resolve(Self::SCREEN_CONTEXT)?,
            draw_image_slot: host.resolve(Self::DRAW_IMAGE)?,
            flush_images_slot: host.resolve(Self::FLUSH_IMAGES)?,
            get_texture_slot: host.resolve(Self::GET_TEXTURE)?,
            gui_data: host.resolve(Self::GUI_DATA)?,
            get_gui_scale: host.resolve(Self::GET_GUI_SCALE)? as usize,
            children: host.resolve(Self::CHILDREN)?,
            cached_position: host.resolve(Self::CACHED_POSITION)?,
            cached_position_dirty: host.resolve(Self::CACHED_POSITION_DIRTY)?,
            cached_position_dirty_mask: host.resolve(Self::CACHED_POSITION_DIRTY_MASK)?,
            visible_in_tree: host.resolve(Self::VISIBLE_IN_TREE)?,
            visible_in_tree_mask: host.resolve(Self::VISIBLE_IN_TREE_MASK)?,
            size: host.resolve(Self::SIZE)?,
            set_cached_position: host.resolve(Self::SET_CACHED_POSITION)? as usize,
            pointer_button_ids: host.resolve(Self::POINTER_BUTTON_IDS)?,
            pointer_location: host.resolve(Self::POINTER_LOCATION)?,
            input_mode: host.resolve(Self::INPUT_MODE).ok(),
            queued_images: host.resolve(Self::QUEUED_IMAGES).ok(),
        })
    }
}
