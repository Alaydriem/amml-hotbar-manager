use amml_sdk::DetourGuard;

use crate::services::HotbarRuntime;

// void ClientNetworkHandler::handle(NetworkIdentifier const&, ItemStackResponsePacket const&): the server's answers
// to item stack requests, the body the live handler's vtable holds. The mod reads them first, as stratum does; the
// game always gets them, once
pub static GUARD: DetourGuard = DetourGuard::new("stack_response");

// this, the source, the packet
type HandleFn = extern "C" fn(usize, usize, usize);

pub extern "C" fn detour(this: usize, source: usize, packet: usize) {
    let original: HandleFn = GUARD.original_fn();
    GUARD.run(
        || {
            if let Some(runtime) = HotbarRuntime::get() {
                runtime.on_item_stack_response(packet);
            }
        },
        || (),
    );
    original(this, source, packet);
}
