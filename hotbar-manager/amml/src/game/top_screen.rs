use std::ffi::c_void;

use amml_game::{GameHeap, RawObject};
use amml_msvc::{MemberCall, MsvcString};

use crate::traits::GameHost;

// Reads the name of the top screen through `ClientInstance::getTopScreenName`, as headless-input does: the name comes
// back through a hidden return pointer, is copied, and its block is released with the game's heap. Read on every client
// tick. Game thread only
pub struct TopScreen {
    slot: u64,
}

impl TopScreen {
    // vtable slot of `std::string IClientInstance::getTopScreenName() const`; it has a recipe
    pub const ENTRY: &'static str = "ClientInstance.getTopScreenName";

    // the reader of the running build; None when the entry is missing, or outside the game (no heap to free a long name)
    pub fn bind<H: GameHost>(host: &H) -> Option<Self> {
        GameHeap::msvc()?;
        host.resolve(Self::ENTRY)
            .inspect_err(|error| {
                host.warn(&format!(
                    "{error}; the screen change is found by the frame gap, which a frame hitch can trip"
                ));
            })
            .ok()
            .map(|slot| Self { slot })
    }

    /// the name of `client`'s top screen; None for a null client or slot, or a name that is not UTF-8
    ///
    /// # Safety
    /// `client` must be null or the live `ClientInstance` of the build the entry was resolved for, on the game thread
    #[allow(unsafe_code)]
    pub unsafe fn read(&self, client: usize) -> Option<String> {
        let client = client as *mut c_void;
        if client.is_null() {
            return None;
        }
        let heap = GameHeap::msvc()?;
        // SAFETY: the client is live, and its vtable holds getTopScreenName at the entry's slot
        let function = unsafe { RawObject::slot(client, self.slot) }?;
        // SAFETY: the slot holds `std::string getTopScreenName() const`, which returns the string through a hidden
        // pointer, and the client is live
        let mut name: MsvcString = unsafe { MemberCall::call_sret0(function, client) };
        let text = name.to_str().ok().map(str::to_owned);
        // SAFETY: the game allocated the string's block, if it has one, with the allocator `heap` mirrors; the string
        // is not used again
        unsafe { name.release(&heap) };
        text
    }
}
