use amml_sdk::{HookHandle, SdkError};

use crate::services::HotbarRuntime;
use crate::traits::GameHost;
use crate::types::HookSpec;

// installs the mod's hooks: every required entry must resolve before any hook goes in; a missing optional
// entry, or an optional hook that does not go in, turns off only its own feature, with one log line
pub struct HookInstaller;

impl HookInstaller {
    // the entries of the hooks that went in, in install order
    pub fn install<H: GameHost>(
        host: &H,
        runtime: &HotbarRuntime,
        specs: &[HookSpec],
    ) -> Result<Vec<&'static str>, SdkError> {
        let mut targets = Vec::with_capacity(specs.len());
        for spec in specs {
            match Self::target(host, spec) {
                Ok(target) => targets.push((target, spec)),
                Err(error) if spec.required => return Err(error),
                Err(error) => {
                    host.warn(&format!("{} did not resolve ({error}); {} is off", spec.entry, spec.feature));
                }
            }
        }
        let mut installed = Vec::with_capacity(targets.len());
        for (target, spec) in targets {
            let kept = match host.hook(target, spec) {
                Ok(hook) => Self::keep(host, runtime, hook),
                Err(error) => Err(error),
            };
            match kept {
                Ok(()) => installed.push(spec.entry),
                Err(error) if spec.required => {
                    Self::remove_all(host, runtime);
                    return Err(error);
                }
                Err(error) => host.warn(&format!("{} was not hooked ({error}); {} is off", spec.entry, spec.feature)),
            }
        }
        Ok(installed)
    }

    // the code address the hook goes on: the entry itself, or the slot `entry` of the vtable `table`
    fn target<H: GameHost>(host: &H, spec: &HookSpec) -> Result<u64, SdkError> {
        let value = host.resolve(spec.entry)?;
        match spec.table {
            Some(table) => host.table_slot(host.resolve(table)?, value),
            None => Ok(value),
        }
    }

    pub fn remove_all<H: GameHost>(host: &H, runtime: &HotbarRuntime) {
        for hook in runtime.take_hooks() {
            if let Err(error) = host.unhook(hook) {
                host.warn(&format!("unhook failed: {error}"));
            }
        }
    }

    // a hook the runtime cannot keep is removed again, so no installed hook is left without a handle
    fn keep<H: GameHost>(host: &H, runtime: &HotbarRuntime, hook: HookHandle) -> Result<(), SdkError> {
        if runtime.keep_hook(hook) {
            return Ok(());
        }
        let _ = host.unhook(hook);
        Err(SdkError::Mod(format!("more than {} hooks", HotbarRuntime::HOOK_CAPACITY)))
    }
}
