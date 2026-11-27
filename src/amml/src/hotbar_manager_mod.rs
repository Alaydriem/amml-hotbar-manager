use std::sync::{Arc, Mutex, PoisonError};

use amml_api::amml_abi::{
    ClientTick, InventoryRequestDone, ModsLoaded, ProviderGone, ProviderReady, ScreenChanged, ScreenRender,
    TexturesReloaded, UiPointer,
};
use amml_api::{AmmlMod, ApiError, Area, Ctx, DisableCtx, Uses};

use crate::services::{HotbarLink, HotbarListeners, HotbarRuntime};

// what the enabled mod owns. `disable` takes it, which drops the listeners and closes the link
static RUNNING: Mutex<Option<HotbarListeners>> = Mutex::new(None);

// the AMML entry point: enable starts the manager link and the listeners, disable removes them and leaves the mod
// manager
pub struct HotbarManagerMod;

impl AmmlMod for HotbarManagerMod {
    const NAMESPACE: &'static str = "alaydriem";
    const NAME: &'static str = "hotbar-manager";
    const USES: Uses = amml_api::amml_uses! {
        feature "core" {
            required ClientTick;
            required ProviderReady;
            required ProviderGone;
            required ModsLoaded;
            required area Area::CLIENT_READ;
        }
        feature "buttons" {
            required ScreenRender;
            required UiPointer;
            required area Area::UI_SCREEN;
            required area Area::UI_DRAW;
            required area Area::UI_TEXTURES;
            required TexturesReloaded;
        }
        feature "screen_actions" {
            required ScreenRender;
            required ScreenChanged;
            required area Area::UI_SCREEN.at(2);
            required area Area::INVENTORY_READ.at(3);
            required area Area::CLIENT_TOP_SCREEN;
        }
        feature "rotation" {
            required InventoryRequestDone;
            required area Area::INVENTORY_READ.at(3);
            required area Area::INVENTORY_REQUEST;
            required area Area::CLIENT_TOP_SCREEN;
        }
    };

    fn enable(ctx: &Ctx) -> Result<(), ApiError> {
        let log = ctx.log();
        let runtime = Arc::new(HotbarRuntime::new(log, ctx.textures()));
        // the buttons and the screen actions do not need the manager: a link that cannot start leaves only the keys off
        match HotbarLink::start(ctx) {
            Ok(link) => runtime.attach(link),
            Err(error) => {
                log.error("the keys are off; the manager link did not start", &[("error", &error.to_string())])
            }
        }
        let listeners = HotbarListeners::start(ctx, runtime).inspect_err(|error| {
            log.error("hotbar-manager off", &[("error", &error.to_string())]);
        })?;
        *RUNNING.lock().unwrap_or_else(PoisonError::into_inner) = Some(listeners);
        Ok(())
    }

    fn disable(_ctx: &Ctx, _how: &DisableCtx) {
        let running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(listeners) = running {
            listeners.stop();
        }
    }
}
