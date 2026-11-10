use std::ffi::c_void;

use amml_game::RawObject;
use amml_msvc::MsvcString;
use hotbar_manager_core::structs::{PanelRect, ScreenKind};

use crate::types::{ButtonCalls, ScreenCalls};

// Finds the classic panel the buttons anchor to, in the view's visual tree (docs/findings/spike-panel/report.md
// section 1): the first control named `root_screen_panel`, then a fixed path of child names under it. The path
// table is pure and tested; the walk reads the tree through the entries of `ScreenCalls` and `ButtonCalls`
pub struct PanelLocator {
    screens: ScreenCalls,
    calls: ButtonCalls,
}

impl PanelLocator {
    pub const CONTENT: &'static str = "root_screen_panel";
    // the walk stops here: a screen tree has a few thousand controls
    pub const MAX_NODES: usize = 20_000;
    // the most children one control is read with
    const MAX_CHILDREN: usize = 4096;
    // `shared_ptr<UIControl>`: the control pointer, then the control block
    const CHILD_STRIDE: usize = 16;

    pub fn new(screens: ScreenCalls, calls: ButtonCalls) -> Self {
        Self { screens, calls }
    }

    // the child names from `root_screen_panel` to the panel of each kind; None where no button anchors
    pub fn path(kind: ScreenKind) -> Option<&'static [&'static str]> {
        match kind {
            ScreenKind::SmallChest
            | ScreenKind::LargeChest
            | ScreenKind::Barrel
            | ScreenKind::ShulkerBox
            | ScreenKind::EnderChest
            | ScreenKind::Hopper
            | ScreenKind::Dispenser
            | ScreenKind::Dropper => Some(&["root_panel"]),
            ScreenKind::PlayerInventory => Some(&["content_stack_panel", "player_inventory"]),
            ScreenKind::Other => None,
        }
    }

    /// the rect of the panel of `kind` in `view`, in GUI units; None when the panel is missing or hidden (the
    /// creative inventory hides the classic one)
    ///
    /// # Safety
    /// `view` must be null or the live `ScreenView` of the build the calls were resolved for, inside its render call
    #[allow(unsafe_code)]
    pub unsafe fn panel(&self, view: *mut c_void, kind: ScreenKind) -> Option<PanelRect> {
        let path = Self::path(kind)?;
        if view.is_null() {
            return None;
        }
        // SAFETY: a live view holds its visual tree, and the tree its root control, at the entries' offsets
        let tree = unsafe { RawObject::pointer(view, self.screens.visual_tree) }?;
        // SAFETY: as above
        let root = unsafe { RawObject::pointer(tree, self.screens.root_control) }?;
        // SAFETY: the root control of a live tree
        let mut control = unsafe { self.find(root, Self::CONTENT) }?;
        for name in path {
            // SAFETY: a live control of this tree
            control = unsafe { self.child(control, name) }?;
        }
        // SAFETY: as above
        if !unsafe { self.visible(control) } {
            return None;
        }
        // SAFETY: as above
        unsafe { self.rect(control) }
    }

    // whether the control's name is `wanted` (no allocation: this runs every frame)
    #[allow(unsafe_code)]
    unsafe fn named(&self, control: *mut c_void, wanted: &str) -> bool {
        let Ok(offset) = usize::try_from(self.screens.control_name) else { return false };
        let address = control.cast::<u8>().wrapping_add(offset);
        if !address.cast::<MsvcString>().is_aligned() {
            return false;
        }
        // SAFETY: a live control holds an MSVC std::string at its mName offset, and the address is aligned
        let name = unsafe { &*address.cast::<MsvcString>() };
        name.as_bytes() == wanted.as_bytes()
    }

    // the children of a live control; empty when the vector does not look like one
    #[allow(unsafe_code)]
    unsafe fn children(&self, control: *mut c_void) -> Vec<*mut c_void> {
        // SAFETY: a live control holds the begin and end pointers of its children vector at the entry's offset
        let begin = unsafe { RawObject::read::<usize>(control, self.calls.children) };
        // SAFETY: as above
        let end = unsafe { RawObject::read::<usize>(control, self.calls.children.saturating_add(8)) };
        let (Some(begin), Some(end)) = (begin, end) else { return Vec::new() };
        let bytes = end.wrapping_sub(begin);
        if begin == 0
            || end < begin
            || bytes % Self::CHILD_STRIDE != 0
            || bytes / Self::CHILD_STRIDE > Self::MAX_CHILDREN
        {
            return Vec::new();
        }
        (0..bytes / Self::CHILD_STRIDE)
            .filter_map(|index| {
                let element = (begin + index * Self::CHILD_STRIDE) as *const c_void;
                // SAFETY: the element is inside the live vector; its first qword is the control pointer
                unsafe { RawObject::pointer(element, 0) }
            })
            .collect()
    }

    // depth-first, the first control named `wanted`
    #[allow(unsafe_code)]
    unsafe fn find(&self, root: *mut c_void, wanted: &str) -> Option<*mut c_void> {
        let mut stack = vec![root];
        let mut seen = 0usize;
        while let Some(control) = stack.pop() {
            seen += 1;
            if seen > Self::MAX_NODES {
                return None;
            }
            // SAFETY: every control on the stack is the root or a child of a live control
            if unsafe { self.named(control, wanted) } {
                return Some(control);
            }
            // SAFETY: as above
            let mut children = unsafe { self.children(control) };
            children.reverse();
            stack.extend(children);
        }
        None
    }

    #[allow(unsafe_code)]
    unsafe fn child(&self, control: *mut c_void, wanted: &str) -> Option<*mut c_void> {
        // SAFETY: a live control; its children are live
        unsafe { self.children(control) }.into_iter().find(|child| unsafe { self.named(*child, wanted) })
    }

    #[allow(unsafe_code)]
    unsafe fn visible(&self, control: *mut c_void) -> bool {
        // SAFETY: a live control holds the flag byte at the entry's offset
        let bits = unsafe { RawObject::read::<u8>(control, self.calls.visible_in_tree) };
        bits.is_some_and(|bits| u64::from(bits) & self.calls.visible_in_tree_mask != 0)
    }

    // the absolute rect, after `_setCachedPosition` when the dirty bit is set (the game's own
    // getAreaOfControlByName does the same)
    #[allow(unsafe_code)]
    unsafe fn rect(&self, control: *mut c_void) -> Option<PanelRect> {
        // SAFETY: a live control holds the flag byte at the entry's offset
        let bits = unsafe { RawObject::read::<u8>(control, self.calls.cached_position_dirty) }?;
        if u64::from(bits) & self.calls.cached_position_dirty_mask != 0 && self.calls.set_cached_position != 0 {
            // SAFETY: the entry is the address of `void UIControl::_setCachedPosition() const`: the control in rcx
            let set: unsafe extern "C" fn(*mut c_void) =
                unsafe { std::mem::transmute(self.calls.set_cached_position as *const c_void) };
            // SAFETY: the control is live, on the game thread inside the render call
            unsafe { set(control) };
        }
        // SAFETY: a live control holds its cached position and size (vec2 each) at the entries' offsets
        let position = unsafe { RawObject::read::<[f32; 2]>(control, self.calls.cached_position) }?;
        // SAFETY: as above
        let size = unsafe { RawObject::read::<[f32; 2]>(control, self.calls.size) }?;
        Some(PanelRect { x: position[0], y: position[1], w: size[0], h: size[1] })
    }
}
