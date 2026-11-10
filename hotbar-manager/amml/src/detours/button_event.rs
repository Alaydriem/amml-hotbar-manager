use amml_sdk::DetourGuard;

use crate::services::HotbarRuntime;

// void ScreenView::handleButtonEvent(uint buttonId, ButtonState state, FocusImpact focus): the game's UI button funnel
// (docs/findings/spike-click/report.md sections 1 and 2). rcx the view, edx the id, r8b 1 press / 0 release, r9b the
// focus impact. The mod decides first; an event it takes never reaches the game (the original is not called)
pub static GUARD: DetourGuard = DetourGuard::new("button_event");

type ButtonEventFn = extern "C" fn(usize, u32, u8, u8);

pub extern "C" fn detour(view: usize, id: u32, state: u8, focus: u8) {
    let taken = GUARD
        .run(|| HotbarRuntime::get().is_some_and(|runtime| runtime.on_button_event(view, id, state != 0)), || false);
    if taken {
        return;
    }
    let original: ButtonEventFn = GUARD.original_fn();
    original(view, id, state, focus);
}
