use amml_sdk::DetourGuard;

use crate::services::HotbarRuntime;

// void ScreenView::render(UIRenderContext&): every view, every frame, on the game thread. The game draws first; the
// mod buttons then go on top through the same context (docs/findings/spike-texture/report.md section 3)
pub static GUARD: DetourGuard = DetourGuard::new("screen_render");

// this (the ScreenView), the MinecraftUIRenderContext
type RenderFn = extern "C" fn(usize, usize);

pub extern "C" fn detour(view: usize, ctx: usize) {
    let original: RenderFn = GUARD.original_fn();
    original(view, ctx);
    GUARD.run(
        || {
            if let Some(runtime) = HotbarRuntime::get() {
                runtime.on_screen_render(view, ctx);
            }
        },
        || (),
    );
}
