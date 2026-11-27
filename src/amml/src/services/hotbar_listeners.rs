use std::sync::Arc;

use amml_api::amml_abi::{ClientTick, InventoryRequestDone, ScreenChanged, ScreenRender, TexturesReloaded, UiPointer};
use amml_api::{ApiError, Ctx, HookOrder, Listener};

use crate::services::HotbarRuntime;

// the loader listeners of the mod; each only hands its event to the runtime. Dropping them removes them
pub struct HotbarListeners {
    listeners: Vec<Listener>,
    runtime: Arc<HotbarRuntime>,
}

impl HotbarListeners {
    // every listen is required by the mod's `USES`: a failure gives the error and the listeners made so far are removed
    pub fn start(ctx: &Ctx, runtime: Arc<HotbarRuntime>) -> Result<Self, ApiError> {
        let events = ctx.events();
        let mut listeners = Vec::with_capacity(6);
        let ticking = Arc::clone(&runtime);
        listeners.push(events.listen::<ClientTick>(HookOrder::NORMAL, move |_, at| {
            ticking.on_tick(at.client().as_ref());
        })?);
        let changing = Arc::clone(&runtime);
        listeners.push(events.listen::<ScreenChanged>(HookOrder::NORMAL, move |event, _| {
            changing.on_screen_changed(event);
        })?);
        let rendering = Arc::clone(&runtime);
        listeners.push(events.listen::<ScreenRender>(HookOrder::NORMAL, move |event, _| {
            rendering.on_screen_render(event);
        })?);
        let pointing = Arc::clone(&runtime);
        listeners.push(events.listen::<UiPointer>(HookOrder::NORMAL, move |event, _| {
            pointing.on_pointer(event);
        })?);
        let finishing = Arc::clone(&runtime);
        listeners.push(events.listen::<InventoryRequestDone>(HookOrder::NORMAL, move |event, _| {
            finishing.on_request_done(event);
        })?);
        // the event is optional: without it a stale texture is found at the next draw
        let reloading = Arc::clone(&runtime);
        if let Ok(listener) = events.listen::<TexturesReloaded>(HookOrder::NORMAL, move |event, _| {
            reloading.on_textures_reloaded(event);
        }) {
            listeners.push(listener);
        }
        Ok(Self { listeners, runtime })
    }

    // the listeners go first, so no listener runs while the runtime lets go of the link
    pub fn stop(self) {
        let Self { listeners, runtime } = self;
        drop(listeners);
        runtime.detach();
    }
}
