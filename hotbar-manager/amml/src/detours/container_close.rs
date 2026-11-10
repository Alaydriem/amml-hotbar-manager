use amml_sdk::DetourGuard;

use crate::services::HotbarRuntime;

// void LegacyClientNetworkHandler::handle(NetworkIdentifier const&, ContainerClosePacket const&): the body the live
// handler's vtable holds at the ContainerClose slot. The server's answer to the rotation's close is hidden from the game
pub static GUARD: DetourGuard = DetourGuard::new("container_close");

// this, the source, the packet
type HandleFn = extern "C" fn(usize, usize, usize);

pub extern "C" fn detour(this: usize, source: usize, packet: usize) {
    let original: HandleFn = GUARD.original_fn();
    let hidden = GUARD.run(|| HotbarRuntime::get().is_some_and(|runtime| runtime.on_container_close(packet)), || false);
    if !hidden {
        original(this, source, packet);
    }
}
