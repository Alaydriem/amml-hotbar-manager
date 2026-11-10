use std::ffi::c_void;

use amml_game::GameHeap;
use amml_sdk::{AmmlMod, Api, ModDirs, SdkError};

use crate::detours::{button_event, client_update, container_close, container_open, screen_render, stack_response};
use crate::game::TopScreen;
use crate::services::{AmmlHost, BoundButtons, HookInstaller, HotbarRuntime, ScreenDetector, SlotReader};
use crate::traits::GameHost;
use crate::types::{
    ButtonCalls, ContainerCalls, GameRead, HookSpec, PacketCalls, PlayerCalls, RotationCalls, ScreenCalls,
    SlotReadCalls,
};

// the AMML entry point: load makes the runtime, enable installs the hooks and binds the item moves to the game,
// disable cancels the moves, removes the hooks and leaves the mod manager. The manager link is made on the first
// ClientInstance::update after enable
pub struct HotbarManagerMod;

impl HotbarManagerMod {
    // the id as the module string the loader and the hook host take; a test checks it against `AmmlMod::id`
    pub const MODULE: &'static str = "@alaydriem/hotbar-manager";
    pub const UPDATE: &'static str = "ClientInstance.update";
    // the item stack response handler: a slot of the live LegacyClientNetworkHandler's vtable, as stratum hooks it
    pub const NETWORK_HANDLER_VTABLE: &'static str = "LegacyClientNetworkHandler.vtable";
    pub const STACK_RESPONSE: &'static str = "ClientNetworkHandler.handle.ItemStackResponsePacket";
    pub const MOVES: &'static str = "the item moves";
    // the mod buttons: the UI button funnel takes their clicks, the view render draws them
    pub const BUTTON_EVENT: &'static str = "ScreenView.handleButtonEvent";
    pub const SCREEN_RENDER: &'static str = "ScreenView.render";
    pub const BUTTONS: &'static str = "the buttons";
    pub const TEXTURES: &'static str = "textures";
    // the hidden inventory open of the row rotation: the server's ContainerOpen and ContainerClose answers are hidden
    // (slots of the live LegacyClientNetworkHandler's vtable, as stratum hooks them)
    pub const CONTAINER_OPEN: &'static str = "ClientNetworkHandler.handle.ContainerOpenPacket";
    pub const CONTAINER_CLOSE: &'static str = "ClientNetworkHandler.handle.ContainerClosePacket";
    pub const ROTATION: &'static str = "the row rotation";

    // every hook, in install order
    pub fn hooks() -> [HookSpec; 6] {
        [
            HookSpec {
                entry: Self::UPDATE,
                table: None,
                detour: client_update::detour as *const c_void,
                original: client_update::GUARD.original(),
                required: true,
                feature: "the mod",
            },
            HookSpec {
                entry: Self::STACK_RESPONSE,
                table: Some(Self::NETWORK_HANDLER_VTABLE),
                detour: stack_response::detour as *const c_void,
                original: stack_response::GUARD.original(),
                required: false,
                feature: Self::MOVES,
            },
            HookSpec {
                entry: Self::BUTTON_EVENT,
                table: None,
                detour: button_event::detour as *const c_void,
                original: button_event::GUARD.original(),
                required: false,
                feature: Self::BUTTONS,
            },
            HookSpec {
                entry: Self::SCREEN_RENDER,
                table: None,
                detour: screen_render::detour as *const c_void,
                original: screen_render::GUARD.original(),
                required: false,
                feature: Self::BUTTONS,
            },
            HookSpec {
                entry: Self::CONTAINER_OPEN,
                table: Some(Self::NETWORK_HANDLER_VTABLE),
                detour: container_open::detour as *const c_void,
                original: container_open::GUARD.original(),
                required: false,
                feature: Self::ROTATION,
            },
            HookSpec {
                entry: Self::CONTAINER_CLOSE,
                table: Some(Self::NETWORK_HANDLER_VTABLE),
                detour: container_close::detour as *const c_void,
                original: container_close::GUARD.original(),
                required: false,
                feature: Self::ROTATION,
            },
        ]
    }

    // every entry the mod resolves; amml.hcl lists each of them
    pub fn entries() -> Vec<&'static str> {
        let hooks = [
            Self::UPDATE,
            Self::NETWORK_HANDLER_VTABLE,
            Self::STACK_RESPONSE,
            Self::BUTTON_EVENT,
            Self::SCREEN_RENDER,
            Self::CONTAINER_OPEN,
            Self::CONTAINER_CLOSE,
        ];
        hooks
            .into_iter()
            .chain(PacketCalls::ENTRIES)
            .chain(PlayerCalls::ENTRIES)
            .chain(ScreenCalls::ENTRIES)
            .chain(SlotReadCalls::ENTRIES)
            .chain(ButtonCalls::ENTRIES)
            .chain(ContainerCalls::ENTRIES)
            .chain(GameRead::ENTRIES)
            .chain([TopScreen::ENTRY, RotationCalls::GET_RUNTIME_ID])
            .collect()
    }

    // the body of enable: the required hook entries must resolve, then the hooks go in and the next tick reaches
    // the manager
    pub fn enable_with<H: GameHost>(host: &H, runtime: &HotbarRuntime) -> Result<(), SdkError> {
        let installed = HookInstaller::install(host, runtime, &Self::hooks())?;
        Self::bind_moves(host, runtime, installed.contains(&Self::STACK_RESPONSE));
        let hooked = installed.contains(&Self::BUTTON_EVENT) && installed.contains(&Self::SCREEN_RENDER);
        Self::bind_buttons(host, runtime, hooked);
        Self::bind_slot_reader(host, runtime);
        Self::bind_screen_reads(host, runtime);
        Self::bind_screen_actions(host, runtime);
        let answered = installed.contains(&Self::CONTAINER_OPEN) && installed.contains(&Self::CONTAINER_CLOSE);
        Self::bind_rotation(host, runtime, answered);
        runtime.enable_link();
        Ok(())
    }

    // the moves need the stack-response hook (without it no answer arrives, and every move would time out after the
    // server may have applied it) and the packet path; the player-slot prediction is optional. A missing entry turns
    // off only its part, with one log line (the hook installer already logged a missing hook)
    fn bind_moves<H: GameHost>(host: &H, runtime: &HotbarRuntime, answers: bool) {
        let packets = if answers {
            PacketCalls::resolve(host).inspect_err(|error| host.warn(&format!("{error}; {} are off", Self::MOVES))).ok()
        } else {
            None
        };
        let players = PlayerCalls::resolve(host)
            .inspect_err(|error| host.warn(&format!("{error}; the client prediction of player slots is off")))
            .ok();
        runtime.bind_moves(packets, players);
    }

    // the buttons need both hooks (a button drawn without the click hook would let its clicks reach the slot under
    // it), the screen detector's entries and their own. Without the input mode entry they still work: a pointer press
    // over a button is taken in every input mode, so it never reaches the game
    fn bind_buttons<H: GameHost>(host: &H, runtime: &HotbarRuntime, hooked: bool) {
        if !hooked {
            runtime.bind_buttons(None);
            return;
        }
        let calls = ScreenCalls::resolve(host).and_then(|screens| Ok((screens, ButtonCalls::resolve(host)?)));
        let bound = match calls {
            Ok((screens, calls)) => {
                if calls.input_mode.is_none() {
                    host.warn(&format!(
                        "{} did not resolve; a pointer press over a button is taken in every input mode",
                        ButtonCalls::INPUT_MODE
                    ));
                }
                if calls.queued_images.is_none() {
                    host.warn(&format!(
                        "{} did not resolve; the buttons draw without checking for images the game left queued",
                        ButtonCalls::QUEUED_IMAGES
                    ));
                }
                Some(BoundButtons::new(screens, calls, runtime.textures_dir()))
            }
            Err(error) => {
                host.warn(&format!("{error}; {} are off", Self::BUTTONS));
                None
            }
        };
        runtime.bind_buttons(bound);
    }

    // the screen reads of the actions: the SDK entries of the view and the container controller. A missing entry turns
    // off only its read, with one log line that names it
    fn bind_screen_reads<H: GameHost>(host: &H, runtime: &HotbarRuntime) {
        let entries = GameRead::resolve(host)
            .inspect_err(|error| host.warn(&format!("{error}; {} is off", GameRead::FEATURE)))
            .ok()
            .filter(|entries| match GameRead::missing(entries) {
                Some(missing) => {
                    host.warn(&format!("{missing} did not resolve; {} is off", GameRead::FEATURE));
                    false
                }
                None => true,
            });
        // the kind of any view, buttons or not, and the top screen name that tells a close or a switch
        let screens = ScreenCalls::resolve(host)
            .inspect_err(|error| host.warn(&format!("{error}; the actions do nothing")))
            .ok()
            .map(ScreenDetector::new);
        runtime.bind_screen_reads(entries, screens, TopScreen::bind(host));
    }

    // the vanilla container handlers of sort, dump and drop. One missing entry turns off the screen actions, with one
    // warning that names it
    fn bind_screen_actions<H: GameHost>(host: &H, runtime: &HotbarRuntime) {
        let calls = ContainerCalls::resolve(host).ok();
        if calls.is_none() {
            let missing = ContainerCalls::missing(host).unwrap_or(ContainerCalls::AUTO_PLACE);
            host.warn(&format!("{missing} did not resolve; the screen actions (sort, dump, drop) are off"));
        }
        runtime.bind_screen_actions(calls);
    }

    // the rotation needs both container hooks (without them the hidden inventory would open on screen, and the close
    // could not be hidden), the top screen name (it says that no screen is open), the player's runtime id for the
    // Interact packet, and the player slots (`PlayerInventory.mInventory`, `FillingContainer.mItems`). Every missing
    // part is named; the rotation is then off, and logs one warning at its first key
    fn bind_rotation<H: GameHost>(host: &H, runtime: &HotbarRuntime, answered: bool) {
        let mut off: Vec<String> = Vec::new();
        if !answered {
            off.push(format!("{} or {} is not hooked", Self::CONTAINER_OPEN, Self::CONTAINER_CLOSE));
        }
        let entries = PlayerCalls::ENTRIES.into_iter().chain([TopScreen::ENTRY, RotationCalls::GET_RUNTIME_ID]);
        off.extend(
            entries.filter(|entry| host.resolve(entry).is_err()).map(|entry| format!("{entry} did not resolve")),
        );
        let calls = RotationCalls::resolve(host).ok();
        let off = (!off.is_empty()).then(|| off.join("; "));
        runtime.bind_rotation(calls.filter(|_| off.is_none()), off);
    }

    // the slot reader needs the game heap, which exists only inside the game (no reader in the tests)
    fn bind_slot_reader<H: GameHost>(host: &H, runtime: &HotbarRuntime) {
        runtime.bind_slot_reader(GameHeap::msvc().and_then(|heap| SlotReader::bind(host, heap)));
    }
}

impl AmmlMod for HotbarManagerMod {
    const NAMESPACE: &'static str = "alaydriem";
    const NAME: &'static str = "hotbar-manager";

    fn load(api: &Api) -> Result<(), SdkError> {
        let dirs = ModDirs::new(api, Self::MODULE)?;
        HotbarRuntime::install(&dirs.config_dir).set_textures_dir(dirs.mod_dir.join(Self::TEXTURES));
        Ok(())
    }

    fn enable(api: &Api) -> Result<(), SdkError> {
        let runtime = HotbarRuntime::get().ok_or_else(|| SdkError::Mod("load did not run".to_string()))?;
        Self::enable_with(&AmmlHost::new(api, Self::MODULE), runtime)
    }

    fn disable(api: &Api) {
        let Some(runtime) = HotbarRuntime::get() else { return };
        runtime.stop_moves();
        runtime.stop_rotation();
        runtime.bind_rotation(None, None);
        runtime.bind_buttons(None);
        runtime.bind_slot_reader(None);
        runtime.bind_screen_reads(None, None, None);
        runtime.bind_screen_actions(None);
        runtime.reset_screen();
        HookInstaller::remove_all(&AmmlHost::new(api, Self::MODULE), runtime);
        runtime.detach();
    }
}
