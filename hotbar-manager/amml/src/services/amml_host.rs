use amml_sdk::amml_abi::AmmlLogLevel;
use amml_sdk::{Api, HookHandle, SdkError};

use crate::traits::GameHost;
use crate::types::HookSpec;

// the AMML loader as the host the mod installs into
pub struct AmmlHost<'a> {
    api: &'a Api,
    module: &'static str,
}

impl<'a> AmmlHost<'a> {
    pub fn new(api: &'a Api, module: &'static str) -> Self {
        Self { api, module }
    }
}

impl GameHost for AmmlHost<'_> {
    fn resolve(&self, id: &str) -> Result<u64, SdkError> {
        self.api.resolve(id)
    }

    fn table_slot(&self, table: u64, slot: u64) -> Result<u64, SdkError> {
        let address = slot.checked_mul(8).and_then(|offset| table.checked_add(offset));
        let Some(address) = address.filter(|_| table != 0) else {
            return Err(SdkError::Mod(format!("slot {slot} of the table {table:#x} is out of range")));
        };
        // SAFETY: `table` is a resolved vtable entry in the game's .rdata and `slot` the resolved slot entry of that
        // class, as stratum reads its network handler slots
        #[allow(unsafe_code)]
        let target = unsafe { (address as *const u64).read() };
        if target == 0 {
            return Err(SdkError::Mod(format!("slot {slot} of the table {table:#x} is empty")));
        }
        Ok(target)
    }

    fn hook(&self, target: u64, spec: &HookSpec) -> Result<HookHandle, SdkError> {
        // SAFETY: `target` is the derived start of the game function `spec.entry` names, and `spec.detour` is the
        // amml_detour! written for that function's signature (this in rcx, then the C++ parameters as the game
        // passes them); HotbarManagerMod::hooks pairs each entry with its detour. Each original slot belongs to one hook
        // only.
        #[allow(unsafe_code)]
        unsafe {
            self.api.hook(target, spec.detour, 0, spec.original)
        }
    }

    fn unhook(&self, hook: HookHandle) -> Result<(), SdkError> {
        self.api.unhook(hook)
    }

    fn warn(&self, message: &str) {
        self.api.log(self.module, AmmlLogLevel::WARN, message);
    }
}
