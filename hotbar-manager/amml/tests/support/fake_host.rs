use std::cell::RefCell;
use std::collections::BTreeMap;

use alaydriem_hotbar_manager::traits::GameHost;
use alaydriem_hotbar_manager::types::HookSpec;
use amml_sdk::{HookHandle, SdkError};

// a loader stand-in: entries it knows resolve, the rest are missing; hooks are recorded, never installed
pub struct FakeHost {
    entries: BTreeMap<&'static str, u64>,
    hooked: RefCell<Vec<&'static str>>,
    targets: RefCell<Vec<u64>>,
    failing: Vec<&'static str>,
    warnings: RefCell<Vec<String>>,
}

impl FakeHost {
    pub fn new(entries: &[(&'static str, u64)]) -> Self {
        Self {
            entries: entries.iter().copied().collect(),
            hooked: RefCell::new(Vec::new()),
            targets: RefCell::new(Vec::new()),
            failing: Vec::new(),
            warnings: RefCell::new(Vec::new()),
        }
    }

    pub fn hooked(&self) -> Vec<&'static str> {
        self.hooked.borrow().clone()
    }

    // a host whose hook on each of `entries` fails
    pub fn failing_hooks(mut self, entries: &[&'static str]) -> Self {
        self.failing = entries.to_vec();
        self
    }

    pub fn hooked_targets(&self) -> Vec<u64> {
        self.targets.borrow().clone()
    }

    // the fake code pointer in `slot` of the vtable at `table`
    pub fn table_target(table: u64, slot: u64) -> u64 {
        0x7000_0000 + table * 0x100 + slot
    }

    pub fn warnings(&self) -> Vec<String> {
        self.warnings.borrow().clone()
    }
}

impl GameHost for FakeHost {
    fn resolve(&self, id: &str) -> Result<u64, SdkError> {
        self.entries.get(id).copied().ok_or_else(|| SdkError::MissingEntry(id.to_string()))
    }

    fn table_slot(&self, table: u64, slot: u64) -> Result<u64, SdkError> {
        Ok(Self::table_target(table, slot))
    }

    fn hook(&self, target: u64, spec: &HookSpec) -> Result<HookHandle, SdkError> {
        if self.failing.contains(&spec.entry) {
            return Err(SdkError::Mod(format!("{} could not be hooked", spec.entry)));
        }
        self.targets.borrow_mut().push(target);
        let mut hooked = self.hooked.borrow_mut();
        hooked.push(spec.entry);
        Ok(HookHandle(hooked.len() as u64))
    }

    fn unhook(&self, _hook: HookHandle) -> Result<(), SdkError> {
        Ok(())
    }

    fn warn(&self, message: &str) {
        self.warnings.borrow_mut().push(message.to_string());
    }
}
