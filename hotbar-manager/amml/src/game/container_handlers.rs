use std::ffi::c_void;

use amml_game::RawObject;
use hotbar_manager_core::structs::{ScreenStep, SlotAddr};

use crate::game::msvc_string::{ItemTransferAmount, MsvcString, SlotData};
use crate::types::ContainerCalls;

// Runs one step of an action on the open screen. False when the step could not be called or the game refused it
// (Swap returns the game's own result)
pub trait ScreenActions {
    fn run(&mut self, step: ScreenStep) -> bool;
}

// The vanilla handlers of one open `ContainerScreenController`, called as the game's own buttons call them: the game
// predicts the move, sends its request and corrects the screen itself. Valid only inside the render call of the
// screen's view, while the controller is live
pub struct ContainerHandlers {
    controller: *mut c_void,
    calls: ContainerCalls,
}

type CollectionCall = unsafe extern "C" fn(*mut c_void, *const MsvcString, i32);
type AutoPlaceCall = unsafe extern "C" fn(*mut c_void, i32, *const MsvcString, i32);
type DropItemCall = unsafe extern "C" fn(*mut c_void, *const MsvcString, i32, *const ItemTransferAmount);
type DropSelectedCall = unsafe extern "C" fn(*mut c_void, *const ItemTransferAmount);
type SwapCall = unsafe extern "C" fn(*mut c_void, *const SlotData, *const SlotData) -> u8;

impl ContainerHandlers {
    // the amount of an auto-place that moves the whole stack
    pub const WHOLE_STACK: i32 = 0x7FFF_FFFF;

    /// None for a null controller or a call address of 0.
    ///
    /// # Safety
    /// `controller` must be the live `ContainerScreenController` of the open screen, and `calls` the entries of the
    /// running build; the handlers must be used only while the controller is live (inside its view's render call)
    #[allow(unsafe_code)]
    pub unsafe fn new(controller: *mut c_void, calls: ContainerCalls) -> Option<Self> {
        let missing = [calls.auto_place, calls.take_all, calls.drop_item, calls.drop_selected].contains(&0);
        // a manager offset of 0 would read the controller's vtable as the manager pointer
        (!controller.is_null() && !missing && calls.manager != 0).then_some(Self { controller, calls })
    }

    fn name(addr: SlotAddr) -> Option<(MsvcString, i32)> {
        let (collection, index) = addr.collection();
        Some((MsvcString::inline(collection.name())?, index))
    }

    fn slot_data(addr: SlotAddr) -> Option<SlotData> {
        let (name, index) = Self::name(addr)?;
        Some(SlotData { name, index })
    }

    // `_handleAutoPlace`: a vanilla shift-click
    #[allow(unsafe_code)]
    fn auto_place(&self, addr: SlotAddr) -> Option<()> {
        let (name, index) = Self::name(addr)?;
        // SAFETY: `ContainerScreenController._handleAutoPlace` is the address of
        // `void(this, int amount, std::string const*, int)`
        let call: AutoPlaceCall = unsafe { std::mem::transmute(self.calls.auto_place as *const c_void) };
        // SAFETY: the controller is live (see `new`); the name outlives the call
        unsafe { call(self.controller, Self::WHOLE_STACK, &name, index) };
        Some(())
    }

    // `_handleTakeAll`: the whole stack of the slot onto the cursor
    #[allow(unsafe_code)]
    fn take_all(&self, addr: SlotAddr) -> Option<()> {
        let (name, index) = Self::name(addr)?;
        // SAFETY: `ContainerScreenController._handleTakeAll` is the address of `void(this, std::string const*, int)`
        let call: CollectionCall = unsafe { std::mem::transmute(self.calls.take_all as *const c_void) };
        // SAFETY: the controller is live (see `new`); the name outlives the call
        unsafe { call(self.controller, &name, index) };
        Some(())
    }

    // `_handlePlaceAll`, a vtable slot of the controller: the cursor stack into the slot
    #[allow(unsafe_code)]
    fn place_all(&self, addr: SlotAddr) -> Option<()> {
        let (name, index) = Self::name(addr)?;
        // SAFETY: the controller is live and its first qword is its vtable, which holds the slot that
        // `ContainerScreenController._handlePlaceAll` names
        let function = unsafe { RawObject::slot(self.controller, self.calls.place_all_slot) }?;
        // SAFETY: the slot holds `void(this, std::string const*, int)`
        let call: CollectionCall = unsafe { std::mem::transmute(function) };
        // SAFETY: the controller is live (see `new`); the name outlives the call
        unsafe { call(self.controller, &name, index) };
        Some(())
    }

    // `ContainerManagerController::handleSwap`, a vtable slot of the manager object: one Swap request, no cursor
    #[allow(unsafe_code)]
    fn swap(&self, a: SlotAddr, b: SlotAddr) -> Option<bool> {
        let (a, b) = (Self::slot_data(a)?, Self::slot_data(b)?);
        // SAFETY: `ContainerScreenController.mContainerManagerController` is the offset of a shared_ptr in the live
        // controller; its first qword is the object pointer
        let manager = unsafe { RawObject::pointer(self.controller, self.calls.manager) }?;
        // SAFETY: the manager object is live while the screen is open, and its first qword is its vtable, which
        // holds the slot that `ContainerManagerController.handleSwap` names
        let function = unsafe { RawObject::slot(manager, self.calls.swap_slot) }?;
        // SAFETY: the slot holds `bool(this, SlotData const*, SlotData const*)`
        let call: SwapCall = unsafe { std::mem::transmute(function) };
        // SAFETY: the manager is live; the slot data outlive the call
        Some(unsafe { call(manager, &a, &b) } != 0)
    }

    // `_handleDropItem`: throw the whole stack of a slot out
    #[allow(unsafe_code)]
    fn drop_slot(&self, addr: SlotAddr) -> Option<()> {
        let (name, index) = Self::name(addr)?;
        let amount = ItemTransferAmount::WHOLE_STACK;
        // SAFETY: `ContainerScreenController._handleDropItem` is the address of
        // `void(this, std::string const*, int, ItemTransferAmount const*)`
        let call: DropItemCall = unsafe { std::mem::transmute(self.calls.drop_item as *const c_void) };
        // SAFETY: the controller is live (see `new`); the name and the amount outlive the call
        unsafe { call(self.controller, &name, index, &amount) };
        Some(())
    }

    // `_handleDropSelectedItem`: throw the cursor stack out
    #[allow(unsafe_code)]
    fn drop_cursor(&self) {
        let amount = ItemTransferAmount::WHOLE_STACK;
        // SAFETY: `ContainerScreenController._handleDropSelectedItem` is the address of
        // `void(this, ItemTransferAmount const*)`
        let call: DropSelectedCall = unsafe { std::mem::transmute(self.calls.drop_selected as *const c_void) };
        // SAFETY: the controller is live (see `new`); the amount outlives the call
        unsafe { call(self.controller, &amount) };
    }
}

impl ScreenActions for ContainerHandlers {
    fn run(&mut self, step: ScreenStep) -> bool {
        match step {
            ScreenStep::AutoPlace(addr) => self.auto_place(addr).is_some(),
            ScreenStep::TakeAll(addr) => self.take_all(addr).is_some(),
            ScreenStep::PlaceAll(addr) => self.place_all(addr).is_some(),
            ScreenStep::Swap(a, b) => self.swap(a, b).unwrap_or(false),
            ScreenStep::DropSlot(addr) => self.drop_slot(addr).is_some(),
            ScreenStep::DropCursor => {
                self.drop_cursor();
                true
            }
        }
    }
}
