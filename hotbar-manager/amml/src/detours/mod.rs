// The game functions the hotbar manager hooks. Each body hands the raw arguments to HotbarRuntime and calls the
// original; bool arguments and results cross as bytes, so no bool invariant is assumed of the game's registers

use amml_sdk::amml_detour;

use crate::services::HotbarRuntime;

pub mod button_event;
pub mod container_close;
pub mod container_open;
pub mod screen_render;
pub mod stack_response;

// bool ClientInstance::update(bool isInitFinished, bool): once per frame on the game thread. The third argument
// (r8) is carried as a raw u64 so the register reaches the original bit for bit
amml_detour!(pub mod client_update(this: usize, init_finished: u8, extra: u64) -> u8 |original| {
    let result = original(this, init_finished, extra);
    if let Some(runtime) = HotbarRuntime::get() {
        runtime.tick(this);
    }
    result
});
