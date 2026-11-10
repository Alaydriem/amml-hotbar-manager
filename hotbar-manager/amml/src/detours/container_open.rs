use amml_sdk::DetourGuard;

use crate::services::HotbarRuntime;

// void LegacyClientNetworkHandler::handle(NetworkIdentifier const&, ContainerOpenPacket const&): the body the live
// handler's vtable holds at the ContainerOpen slot. The inventory open the rotation asked for is hidden from the game
pub static GUARD: DetourGuard = DetourGuard::new("container_open");

// this, the source, the packet
type HandleFn = extern "C" fn(usize, usize, usize);

pub extern "C" fn detour(this: usize, source: usize, packet: usize) {
    let original: HandleFn = GUARD.original_fn();
    // the original runs for every open the rotation did not ask for, and when the body panics
    let hidden = GUARD.run(|| HotbarRuntime::get().is_some_and(|runtime| runtime.on_container_open(packet)), || false);
    if !hidden {
        original(this, source, packet);
    }
}
