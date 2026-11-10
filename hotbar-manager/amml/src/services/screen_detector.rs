use std::ffi::c_void;

use amml_game::RawObject;
use amml_msvc::MsvcString;
use hotbar_manager_core::structs::ScreenKind;

use crate::types::{ScreenCalls, UiProfile};

// Finds the kind of the open screen and the UI profile. The name table and the profile mapping are pure and tested;
// the game reads are a thin wrapper over the entries of `ScreenCalls`
pub struct ScreenDetector {
    calls: ScreenCalls,
}

impl ScreenDetector {
    pub fn new(calls: ScreenCalls) -> Self {
        Self { calls }
    }

    // the kind of the screen whose root control is named `name` (`UIScene::getRawScreenName`, no namespace), from
    // docs/findings/spike-panel/report.md section 3. Names that report does not record give Other; the dropper's
    // `dropper_screen` is recorded, the crafting screen, the trapped chest and the minecarts are not
    pub fn kind_of(name: &str) -> ScreenKind {
        match name {
            "small_chest_screen" => ScreenKind::SmallChest,
            "large_chest_screen" => ScreenKind::LargeChest,
            "barrel_screen" => ScreenKind::Barrel,
            "shulker_box_screen" => ScreenKind::ShulkerBox,
            "ender_chest_screen" => ScreenKind::EnderChest,
            "hopper_screen" => ScreenKind::Hopper,
            "dispenser_screen" => ScreenKind::Dispenser,
            "dropper_screen" => ScreenKind::Dropper,
            "inventory_screen" | "inventory_screen_pocket" => ScreenKind::PlayerInventory,
            _ => ScreenKind::Other,
        }
    }

    /// the screen kind of `view` and the profile of `client`'s options. A view that cannot be read gives Other; a
    /// client that cannot be read gives Classic
    ///
    /// # Safety
    /// `view` must be null or the live `ScreenView`, and `client` null or the live `ClientInstance`, of the build the
    /// calls were resolved for
    #[allow(unsafe_code)]
    pub unsafe fn detect(&self, view: *mut c_void, client: *mut c_void) -> (ScreenKind, UiProfile) {
        // SAFETY: forwarded to the caller
        let kind = unsafe { self.screen_kind(view) }.unwrap_or(ScreenKind::Other);
        // SAFETY: forwarded to the caller
        let profile = unsafe { self.profile(client) }.map_or(UiProfile::Classic, UiProfile::from_value);
        (kind, profile)
    }

    // the kind of the view's root control, by its name (no allocation: this runs every frame)
    #[allow(unsafe_code)]
    unsafe fn screen_kind(&self, view: *mut c_void) -> Option<ScreenKind> {
        if view.is_null() {
            return None;
        }
        // SAFETY: a live view holds its visual tree at the entry's offset; the tree holds the root control pointer
        // and the control holds a std::string name, at the entries' offsets
        let tree = unsafe { RawObject::pointer(view, self.calls.visual_tree) }?;
        // SAFETY: as above
        let root = unsafe { RawObject::pointer(tree, self.calls.root_control) }?;
        let offset = usize::try_from(self.calls.control_name).ok()?;
        let address = root.cast::<u8>().wrapping_add(offset);
        if !address.cast::<MsvcString>().is_aligned() {
            return None;
        }
        // SAFETY: the live control holds an MSVC std::string at its mName offset, and the address is aligned
        let name = unsafe { &*address.cast::<MsvcString>() };
        name.to_str().ok().map(Self::kind_of)
    }

    // the raw value of `getUIProfile` on the client's options
    #[allow(unsafe_code)]
    unsafe fn profile(&self, client: *mut c_void) -> Option<i32> {
        if client.is_null() {
            return None;
        }
        // SAFETY: the live client, whose vtable holds getOptions at the entry's slot
        let get_options = unsafe { RawObject::slot(client, self.calls.get_options_slot) }?;
        // SAFETY: the slot holds `IOptionRegistry& getOptions()`
        let options = unsafe { RawObject::call_pointer(get_options, client) }?;
        // SAFETY: the options object, whose vtable holds getUIProfile at the entry's slot
        let function = unsafe { RawObject::slot(options, self.calls.get_ui_profile_slot) }?;
        // SAFETY: the slot holds `UIProfile getUIProfile() const`: this in rcx, the int enum in eax
        let call: unsafe extern "C" fn(*mut c_void) -> i32 = unsafe { std::mem::transmute(function) };
        // SAFETY: the options object is live
        Some(unsafe { call(options) })
    }
}
