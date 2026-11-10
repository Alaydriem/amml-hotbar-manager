use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, TryLockError};
use std::time::Instant;

use amm_abi::types::AmmStatus;
use amm_client::link::ApiTable;
use amm_core::errors::LinkError;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amml_game::{ContainerClosePacketLayout, ContainerOpenPacketLayout, GameEntries, ItemStackResponsePacketLayout};
use amml_sdk::{HookHandle, ModRuntime};
use hotbar_manager_core::services::SortTick;
use hotbar_manager_core::structs::{ButtonAction, Direction, ScreenKind};

use crate::game::{
    ContainerHandlers, GamePacket, GameScreen, HiddenInventory, PacketId, PlayerPredictor, PlayerSlots, ScreenActions,
    StackResponses, TopScreen,
};
use crate::services::{
    ActionResult, AnswerGate, BoundButtons, ButtonUi, FeatureRunner, HotbarLink, Ignored, MoveEngine, RotationEnd,
    RotationIgnored, RotationResult, RotationRunner, ScreenDetector, ScreenGuard, ScreenWatch, SlotReader, SortEnd,
    Verdict,
};
use crate::traits::{InventoryLink, ScreenSource};
use crate::types::{ClickRoute, ContainerCalls, LinkState, PacketCalls, PlayerCalls, RotationCalls};

// what the hooks share: the manager link state, the installed hooks, the screen actions and the row rotation's move
// engine. The mod manager is reached on the first ClientInstance::update after enable, when every mod of the host has
// loaded; `hooks` holds at most HOOK_CAPACITY handles. The screen actions (sort, dumps, drops) call the game's own
// container handlers inside the ScreenView::render detour; only the rotation sends raw ItemStackRequests. The server's
// answers wait in `answers` until the engine's lock is free, so an answer that comes while a request is being sent (a
// local world may answer at once) is not lost
pub struct HotbarRuntime {
    config_dir: PathBuf,
    link_state: AtomicU8,
    link: Mutex<Option<HotbarLink>>,
    hooks: Mutex<Vec<HookHandle>>,
    // the move engine of the row rotation
    moves: Mutex<MoveEngine<PlayerPredictor>>,
    answers: Mutex<Vec<(i32, bool)>>,
    packet_calls: Mutex<Option<PacketCalls>>,
    // the ClientInstance of the last tick
    client: AtomicUsize,
    // the mod's textures folder (`<mod folder>/textures`), set at load
    textures_dir: Mutex<Option<PathBuf>>,
    // the mod buttons: drawn by the ScreenView::render detour, clicked through the handleButtonEvent detour
    buttons: Mutex<ButtonUi>,
    // the slot reader of the open screen, for the button actions (task 12)
    slot_reader: Mutex<Option<SlotReader>>,
    // the busy-buttons warning was logged
    busy_warned: AtomicBool,
    // the feature layer: the steps of an action, and the sort that runs
    runner: Mutex<FeatureRunner>,
    // the button action that waits for the next frame of the open screen: the view is read only inside its own
    // render call, where it is live
    pending: Mutex<Option<ButtonAction>>,
    // the identity of the open screen (the top screen name of each tick, the controller of its frames) that the
    // queued or running action is bound to
    guard: Mutex<ScreenGuard>,
    // reads the top screen name on every tick; None when its entry is missing (the frame gap below takes over)
    top_screen: Mutex<Option<TopScreen>>,
    // finds the kind of the screen a frame belongs to, with or without buttons
    detector: Mutex<Option<ScreenDetector>>,
    // the last frame that drew buttons: the fallback signal while the top screen name cannot be read
    watch: Mutex<ScreenWatch>,
    // the SDK entries the screen reads use; missing, the screen reads are off
    game_entries: Mutex<Option<Arc<GameEntries>>>,
    // the vanilla container handlers of the screen actions; None when one of their entries is missing
    container_calls: Mutex<Option<ContainerCalls>>,
    // the one warning that the screen reads are off
    screen_warned: AtomicBool,
    // the rotation: a hidden inventory open that runs the row moves; the calls it needs beyond the moves, and why it is
    // off (None when it is on)
    rotation: Mutex<RotationRunner>,
    rotation_calls: Mutex<Option<RotationCalls>>,
    player_calls: Mutex<Option<PlayerCalls>>,
    rotation_off: Mutex<Option<String>>,
    rotation_warned: AtomicBool,
    // hides, while the runner's lock is busy, only an answer to the Interact or close send in progress
    gate: AnswerGate,
    // the clock of the engine and the watch
    started: Instant,
    // the client ticks so far, and 1 + the tick of the last sort step (0: none). Each container screen renders a
    // second view of the same controller in every frame, and a tick has several frames; a sort takes at most one step
    // per tick, so the next step always reads the screen after the game has handled the last one
    ticks: AtomicU64,
    sort_step_tick: AtomicU64,
}

static RUNTIME: OnceLock<HotbarRuntime> = OnceLock::new();

impl HotbarRuntime {
    pub const HOOK_CAPACITY: usize = 8;
    // the keybind actions of the descriptor, in the order it declares them: rotate_up, rotate_down, sort
    pub const SORT_KEYBIND: u16 = 2;
    // the rotation keybinds, in the same order: rotate_up, rotate_down
    pub const ROTATE_UP_KEYBIND: u16 = 0;
    pub const ROTATE_DOWN_KEYBIND: u16 = 1;

    pub fn new(config_dir: &Path) -> Self {
        Self {
            config_dir: config_dir.to_path_buf(),
            link_state: AtomicU8::new(LinkState::Waiting.to_u8()),
            link: Mutex::new(None),
            hooks: Mutex::new(Vec::with_capacity(Self::HOOK_CAPACITY)),
            moves: Mutex::new(MoveEngine::new(PlayerPredictor::new())),
            answers: Mutex::new(Vec::new()),
            packet_calls: Mutex::new(None),
            client: AtomicUsize::new(0),
            textures_dir: Mutex::new(None),
            buttons: Mutex::new(ButtonUi::new()),
            slot_reader: Mutex::new(None),
            busy_warned: AtomicBool::new(false),
            runner: Mutex::new(FeatureRunner::new()),
            pending: Mutex::new(None),
            guard: Mutex::new(ScreenGuard::new()),
            top_screen: Mutex::new(None),
            detector: Mutex::new(None),
            watch: Mutex::new(ScreenWatch::new()),
            game_entries: Mutex::new(None),
            container_calls: Mutex::new(None),
            screen_warned: AtomicBool::new(false),
            rotation: Mutex::new(RotationRunner::new()),
            rotation_calls: Mutex::new(None),
            player_calls: Mutex::new(None),
            rotation_off: Mutex::new(None),
            rotation_warned: AtomicBool::new(false),
            gate: AnswerGate::new(),
            started: Instant::now(),
            ticks: AtomicU64::new(0),
            sort_step_tick: AtomicU64::new(0),
        }
    }

    // the runtime of this DLL; the first call wins
    pub fn install(config_dir: &Path) -> &'static HotbarRuntime {
        RUNTIME.get_or_init(|| Self::new(config_dir))
    }

    pub fn get() -> Option<&'static HotbarRuntime> {
        RUNTIME.get()
    }

    pub fn link_state(&self) -> LinkState {
        LinkState::from_u8(self.link_state.load(Ordering::SeqCst))
    }

    // enable: the next tick reaches the manager again
    pub fn enable_link(&self) {
        self.link_state.store(LinkState::Waiting.to_u8(), Ordering::SeqCst);
    }

    // once per ClientInstance::update on the game thread, after the original, with the client: reaches the manager on
    // the first call after enable, then reads its events (the keybinds), and drives the row rotation while it runs. An
    // action that outlives its screen is dropped here
    pub fn tick(&self, client: usize) {
        self.next_tick();
        self.client.store(client, Ordering::SeqCst);
        if self.link_state() == LinkState::Waiting {
            self.attach_with(ApiTable::resolve());
        }
        let now = self.now();
        if self.link_state() == LinkState::Attached {
            let events = {
                let mut slot = self.link.lock().unwrap_or_else(PoisonError::into_inner);
                slot.as_mut().map(HotbarLink::drain).unwrap_or_default()
            };
            for event in &events {
                self.on_event(event, now);
            }
        }
        // SAFETY: the client of this tick, live, on the game thread
        #[allow(unsafe_code)]
        let top = self
            .top_screen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|reader| unsafe { reader.read(client) });
        match top {
            Some(Some(name)) => self.on_top_screen(&name, now),
            // the reader is bound but this read failed: nothing is known, nothing changes
            Some(None) => {}
            // no reader: the frame gap is the close signal
            None => self.sweep(now),
        }
        // a rotation drives the engine itself (its slots come from the player's inventory, not from a screen frame)
        if self.rotation_active() {
            self.drive_rotation(now);
        }
    }

    // the top screen of this tick. When it differs from the screen the queued or running action started on (a close,
    // a switch), the sort and the queued action are dropped at once. Time does not matter here: a frame
    // hitch changes nothing
    pub fn on_top_screen(&self, name: &str, _now: f64) {
        if self.lock_guard().top_changed(name) == Verdict::Cancel {
            self.abort_screen();
        }
    }

    // a frame of the open screen drawn by the controller at `controller`; false (the action is cancelled and dropped)
    // when that is not the controller the action started on
    pub fn accept_frame(&self, controller: usize) -> bool {
        if self.lock_guard().frame(controller) == Verdict::Cancel {
            self.abort_screen();
            return false;
        }
        true
    }

    // the screen the action belongs to is gone: the sort stops and nothing waits; no call reaches its controller
    fn abort_screen(&self) {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let ended = self.lock_runner().stop_sort(FeatureRunner::SCREEN_GONE);
        self.lock_guard().release();
        // only a sort that ran logs its end: an idle sweep logs nothing
        if let Some(end) = ended {
            Self::log_sort_end(end);
        }
    }

    // a new client tick: the sort may take its next step
    pub fn next_tick(&self) {
        self.ticks.fetch_add(1, Ordering::SeqCst);
    }

    fn lock_guard(&self) -> std::sync::MutexGuard<'_, ScreenGuard> {
        self.guard.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // seconds since the runtime was made: the clock of the rotation, the engine and the watch
    pub fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    // one event of the manager: a keybind, pressed (not a repeat or a release). The sort keybind runs the sort, the
    // rotation keybinds rotate the rows
    pub fn on_event(&self, event: &ModEvent, now: f64) {
        let ModEvent::Keybind { action, phase: KeybindPhase::Pressed } = event else { return };
        match *action {
            Self::SORT_KEYBIND => self.on_action(ButtonAction::Sort, now),
            Self::ROTATE_UP_KEYBIND => self.on_rotate(Direction::Up, now),
            Self::ROTATE_DOWN_KEYBIND => self.on_rotate(Direction::Down, now),
            _ => {}
        }
    }

    // a screen that is not drawing any more (closed, or changed to one with no buttons) takes with it the action that
    // waited for a frame, and the sort that runs
    pub fn sweep(&self, now: f64) {
        if self.watch.lock().unwrap_or_else(PoisonError::into_inner).open_view(now).is_some() {
            return;
        }
        self.abort_screen();
    }

    // registers with the manager the table belongs to; on failure the mod stays off and one line is logged.
    // Only a Waiting runtime takes the result: if disable ran meanwhile, a new link is dropped again
    pub fn attach_with(&self, table: Result<ApiTable, LinkError>) -> LinkState {
        let state = match table.and_then(|table| HotbarLink::new(table, &self.config_dir)) {
            Ok(link) => {
                *self.link.lock().unwrap_or_else(PoisonError::into_inner) = Some(link);
                curia::info!("@alaydriem/hotbar-manager attached to the mod manager");
                LinkState::Attached
            }
            Err(error) => {
                Self::log_link_error(&error);
                LinkState::Off
            }
        };
        let waiting = LinkState::Waiting.to_u8();
        match self.link_state.compare_exchange(waiting, state.to_u8(), Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => state,
            Err(current) => {
                if state == LinkState::Attached {
                    drop(self.link.lock().unwrap_or_else(PoisonError::into_inner).take());
                }
                LinkState::from_u8(current)
            }
        }
    }

    // disable: the mod unregisters; no tick reaches the manager until the next enable
    pub fn detach(&self) {
        self.link_state.store(LinkState::Disabled.to_u8(), Ordering::SeqCst);
        drop(self.link.lock().unwrap_or_else(PoisonError::into_inner).take());
    }

    // keeps an installed hook; false, keeping nothing, when HOOK_CAPACITY hooks are already kept
    pub fn keep_hook(&self, hook: HookHandle) -> bool {
        let mut hooks = self.hooks.lock().unwrap_or_else(PoisonError::into_inner);
        if hooks.len() >= Self::HOOK_CAPACITY {
            return false;
        }
        hooks.push(hook);
        true
    }

    // the installed hooks, newest first, leaving none behind
    pub fn take_hooks(&self) -> Vec<HookHandle> {
        let mut hooks = self.hooks.lock().unwrap_or_else(PoisonError::into_inner);
        let mut taken = std::mem::take(&mut *hooks);
        taken.reverse();
        taken
    }

    // enable: the packet path the moves send through, and the player calls of the prediction. Without the packet
    // path no move is sent; without the player calls nothing is predicted
    pub fn bind_moves(&self, packets: Option<PacketCalls>, players: Option<PlayerCalls>) {
        *self.packet_calls.lock().unwrap_or_else(PoisonError::into_inner) = packets;
        *self.player_calls.lock().unwrap_or_else(PoisonError::into_inner) = players;
        self.lock_moves().predictor_mut().bind(players);
    }

    // whether the rotation's moves can be sent: the stack-response hook is in and the packet path resolved
    pub fn moves_ready(&self) -> bool {
        self.packet_calls.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    // runs `body` on the rotation's move engine, with the answers that came meanwhile given to it first
    pub fn with_moves<R>(&self, body: impl FnOnce(&mut MoveEngine<PlayerPredictor>) -> R) -> R {
        let mut engine = self.lock_moves();
        self.drain_answers(&mut engine);
        body(&mut engine)
    }

    // the rotation ended without its client, or the mod is disabled: the move in flight stops
    pub fn cancel_moves(&self) {
        let mut engine = self.lock_moves();
        engine.cancel();
        self.answers.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    // disable, which may run off the game thread: the moves are turned off and the predictor unbound first, so the
    // cancel undoes no prediction through the game (the next sync puts the client view right)
    pub fn stop_moves(&self) {
        self.bind_moves(None, None);
        self.cancel_moves();
    }

    // the body of the ItemStackResponse handler detour, before the game's own handler: the answers are kept, then
    // given to the engine when its lock is free
    pub fn on_item_stack_response(&self, packet: usize) {
        let pointer = packet as *const c_void;
        if pointer.is_null() {
            return;
        }
        // SAFETY: the handler's live packet, whose vtable starts with Packet's virtuals
        #[allow(unsafe_code)]
        let name = unsafe { GamePacket::name_of(pointer) };
        if name.as_deref() != Some(PacketId::ITEM_STACK_RESPONSE.name) {
            curia::warn!("the stack response hook got another packet; it is not read", { found: name });
            return;
        }
        // SAFETY: the handler's live ItemStackResponsePacket (its name says so), laid out as the mirror
        #[allow(unsafe_code)]
        let Some(packet) = (unsafe { pointer.cast::<ItemStackResponsePacketLayout>().as_ref() }) else { return };
        let answers = StackResponses::answers(packet);
        self.answers.lock().unwrap_or_else(PoisonError::into_inner).extend(answers);
        if let Ok(mut engine) = self.moves.try_lock() {
            self.drain_answers(&mut engine);
        }
    }

    // load: where the button glyphs are
    pub fn set_textures_dir(&self, dir: PathBuf) {
        *self.textures_dir.lock().unwrap_or_else(PoisonError::into_inner) = Some(dir);
    }

    // the textures folder as getTexture takes a Raw path: absolute, forward slashes
    pub fn textures_dir(&self) -> Option<String> {
        let dir = self.textures_dir.lock().unwrap_or_else(PoisonError::into_inner);
        dir.as_ref().map(|dir| dir.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string())
    }

    // enable: the game side of the buttons, or None to turn them off (disable, or an entry or hook is missing)
    pub fn bind_buttons(&self, bound: Option<BoundButtons>) {
        self.lock_buttons().bind(bound);
    }

    // whether the buttons are drawn and take clicks
    pub fn buttons_ready(&self) -> bool {
        self.lock_buttons().bound()
    }

    // runs `body` on the buttons (the tests drive the routing through it)
    pub fn with_buttons<R>(&self, body: impl FnOnce(&mut ButtonUi) -> R) -> R {
        body(&mut self.lock_buttons())
    }

    // enable: the slot reader the button actions read the screen with; None when its entries are missing
    pub fn bind_slot_reader(&self, reader: Option<SlotReader>) {
        *self.slot_reader.lock().unwrap_or_else(PoisonError::into_inner) = reader;
    }

    pub fn slot_reader_ready(&self) -> bool {
        self.slot_reader.lock().unwrap_or_else(PoisonError::into_inner).is_some()
    }

    // the body of the ScreenView::render detour, after the game drew the view: the buttons go on top. A view that drew
    // buttons is an open screen; inside this call the view and its controller are live, so this is where the screen is
    // read and the container handlers are called: the action that waits runs, and a running sort takes its next step
    pub fn on_screen_render(&self, view: usize, ctx: usize) {
        let client = self.client.load(Ordering::SeqCst);
        let drew = {
            let Some(mut buttons) = self.try_lock_buttons() else { return };
            // SAFETY: the detour's live view and context, inside the render call on the game thread; the client is
            // the one of the last ClientInstance::update, live while the game runs
            #[allow(unsafe_code)]
            unsafe {
                buttons.on_render(view, ctx, client)
            };
            buttons.layout(view).is_some()
        };
        let now = self.now();
        if drew {
            self.note_frame(view, now);
        }
        // the action works with or without drawn buttons (Pocket, unbound buttons): the kind comes from the view
        self.on_frame(view, client);
    }

    // a frame of the open screen, inside the render call of `view` and after the game drew it: the screen and the
    // container handlers of its live controller, given to `run_frame`. Nothing is read or called when no action waits
    // and no sort runs
    fn on_frame(&self, view: usize, client: usize) {
        if self.pending_action().is_none() && !self.sort_running() {
            return;
        }
        let Some(kind) = self.kind_of_view(view, client) else { return };
        if kind == ScreenKind::Other {
            return;
        }
        let entries = self.game_entries.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let reader = self.slot_reader.lock().unwrap_or_else(PoisonError::into_inner);
        let (Some(entries), Some(reader)) = (entries, reader.as_ref()) else {
            self.warn_screen_off();
            return;
        };
        // SAFETY: inside the render call of `view`, which is live; the entries were resolved for this build at enable
        #[allow(unsafe_code)]
        let screen = unsafe { GameScreen::new(&entries, reader, view, kind) };
        // the handlers take the view's ContainerScreenController: the object the slot reads call
        // getVisualItemStackImpl on
        let controller = screen.container_controller();
        let calls = self.container_calls();
        // SAFETY: the controller of the live view, inside its render call on the game thread; the calls were resolved
        // for this build at enable. The handlers are dropped at the end of this call, before the frame ends
        #[allow(unsafe_code)]
        let mut handlers = match (controller, calls) {
            (Some(controller), Some(calls)) => unsafe { ContainerHandlers::new(controller, calls) },
            _ => None,
        };
        let actions = handlers.as_mut().map(|handlers| handlers as &mut dyn ScreenActions);
        self.run_frame(&screen, screen.controller_address(), actions);
    }

    // one frame of the open screen drawn by the controller at `controller`, with the container handlers of that
    // controller (None when they cannot be called). A frame of another controller than the action's drops it. The
    // action that waits runs: a dump or a drop calls all its steps now, a sort starts. A running sort takes one step,
    // at most one per client tick (the first one in the frame it starts). The game thread calls this only from
    // `on_frame`; the tests call it with fakes.
    //
    // The runner's lock is held while the game's container handlers run. No hooked function that takes it runs inside
    // those handlers today; a new hook on a path they reach must not call `on_action`, `sort_running` or `run_frame`,
    // or the game thread deadlocks (std `Mutex` is not re-entrant)
    pub fn run_frame(
        &self,
        screen: &dyn ScreenSource,
        controller: Option<usize>,
        mut actions: Option<&mut dyn ScreenActions>,
    ) {
        if self.pending_action().is_none() && !self.sort_running() {
            return;
        }
        // the action belongs to the controller of its first frame; a frame of another controller is another screen
        if let Some(controller) = controller
            && !self.accept_frame(controller)
        {
            return;
        }
        let action = self.pending.lock().unwrap_or_else(PoisonError::into_inner).take();
        let mut runner = self.lock_runner();
        if let Some(action) = action {
            let result = match actions.as_deref_mut() {
                Some(actions) => runner.start(action, screen.kind(), screen, actions),
                None => ActionResult::Ignored(Ignored::ActionsOff),
            };
            if !matches!(result, ActionResult::Started { .. }) {
                self.lock_guard().release();
            }
            self.log_result(action, result);
        }
        if !runner.sorting() {
            return;
        }
        let Some(actions) = actions else {
            let ended = runner.stop_sort(FeatureRunner::NO_HANDLERS);
            self.lock_guard().release();
            if let Some(end) = ended {
                Self::log_sort_end(end);
            }
            return;
        };
        // one step per tick: the second view of the same controller, and the later frames of this tick, wait
        let tick = self.ticks.load(Ordering::SeqCst).wrapping_add(1);
        if self.sort_step_tick.swap(tick, Ordering::SeqCst) == tick {
            return;
        }
        if let Some(SortTick::Done | SortTick::Stuck) = runner.step_sort(screen, actions) {
            self.lock_guard().release();
            if let Some(end) = runner.take_sort_end() {
                Self::log_sort_end(end);
            }
        }
    }

    // whether the one warning that the row rotation is off was given
    pub fn rotation_off_warned(&self) -> bool {
        self.rotation_warned.load(Ordering::SeqCst)
    }

    // whether a sort runs
    pub fn sort_running(&self) -> bool {
        self.lock_runner().sorting()
    }

    // the kind of the screen `view` shows, whether or not it drew buttons
    fn kind_of_view(&self, view: usize, client: usize) -> Option<ScreenKind> {
        let detector = self.detector.lock().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: inside the render call of `view`, which is live; the client is the one of the last
        // ClientInstance::update, live while the game runs
        #[allow(unsafe_code)]
        detector.as_ref().map(|detector| unsafe { detector.detect(view as *mut c_void, client as *mut c_void) }.0)
    }

    // the body of the ScreenView::handleButtonEvent detour: true when the game must not get the event. A release
    // inside the pressed button runs its action, after the buttons' lock is released
    pub fn on_button_event(&self, view: usize, id: u32, pressed: bool) -> bool {
        let Some(mut buttons) = self.try_lock_buttons() else { return false };
        // SAFETY: the detour's live view, on the game thread
        #[allow(unsafe_code)]
        let route = unsafe { buttons.on_button_event(view, id, pressed) };
        drop(buttons);
        if let Some(action) = ButtonUi::fired(route) {
            self.on_action(action, self.now());
        }
        route != ClickRoute::Pass
    }

    // the one place a button action or a keybind arrives. The screen is read and its handlers called inside its own
    // render call, where its view is live, so the action waits for the next frame of the open screen (`run_frame`).
    // An action while the container handlers are off, an action already waiting, a running sort or a running rotation
    // is ignored. A screen that stops drawing drops the waiting action (`sweep`)
    pub fn on_action(&self, action: ButtonAction, now: f64) {
        if !self.screen_actions_ready() {
            self.log_result(action, ActionResult::Ignored(Ignored::ActionsOff));
            return;
        }
        // a screen is open when the top screen name says so (Pocket and unbound buttons included); without the name,
        // when a view drew buttons lately
        let (known, menu, kind) = {
            let guard = self.lock_guard();
            (guard.top_known(), guard.menu_open(), guard.top_kind())
        };
        let open = if known {
            menu
        } else {
            self.watch.lock().unwrap_or_else(PoisonError::into_inner).open_view(now).is_some()
        };
        if !open {
            self.log_result(action, ActionResult::Ignored(Ignored::NoScreen));
            return;
        }
        // a screen the mod knows by name that the action does not fit is refused now; an unknown name is checked at the
        // frame, by the view
        if kind != ScreenKind::Other && !FeatureRunner::fits(action, kind) {
            self.log_result(action, ActionResult::Ignored(Ignored::WrongScreen));
            return;
        }
        if self.sort_running() || self.rotation_active() {
            self.log_result(action, ActionResult::Ignored(Ignored::Busy));
            return;
        }
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        if pending.is_some() {
            self.log_result(action, ActionResult::Ignored(Ignored::Busy));
            return;
        }
        *pending = Some(action);
        drop(pending);
        self.lock_guard().bind();
    }

    // the action that waits for the next frame
    pub fn pending_action(&self) -> Option<ButtonAction> {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // a frame of the open screen was drawn at `now` by `view`, as `on_screen_render` records it
    pub fn note_frame(&self, view: usize, now: f64) {
        self.watch.lock().unwrap_or_else(PoisonError::into_inner).seen(view, now);
    }

    // enable: the SDK entries the screen reads use (None when one is missing), the screen detector and the top screen
    // reader. A missing read is warned once when an action needs it
    pub fn bind_screen_reads(
        &self,
        entries: Option<GameEntries>,
        screens: Option<ScreenDetector>,
        top: Option<TopScreen>,
    ) {
        *self.detector.lock().unwrap_or_else(PoisonError::into_inner) = screens;
        *self.top_screen.lock().unwrap_or_else(PoisonError::into_inner) = top;
        *self.game_entries.lock().unwrap_or_else(PoisonError::into_inner) = entries.map(Arc::new);
        self.screen_warned.store(false, Ordering::SeqCst);
    }

    // enable: the calls of the vanilla container handlers; None when one of their entries is missing
    pub fn bind_screen_actions(&self, calls: Option<ContainerCalls>) {
        *self.container_calls.lock().unwrap_or_else(PoisonError::into_inner) = calls;
    }

    pub fn screen_actions_ready(&self) -> bool {
        self.container_calls().is_some()
    }

    pub fn container_calls(&self) -> Option<ContainerCalls> {
        *self.container_calls.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // disable: nothing waits, no sort runs, nothing is kept of the screen
    pub fn reset_screen(&self) {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let mut runner = self.lock_runner();
        let _ = runner.stop_sort(FeatureRunner::SCREEN_GONE);
        let _ = runner.take_sort_end();
        drop(runner);
        self.watch.lock().unwrap_or_else(PoisonError::into_inner).clear();
        *self.lock_guard() = ScreenGuard::new();
    }

    // enable: the rotation's own calls, or None with the reason it is off (every missing entry or hook, named). The
    // reason is logged once, at the first rotation key
    pub fn bind_rotation(&self, calls: Option<RotationCalls>, off: Option<String>) {
        *self.rotation_calls.lock().unwrap_or_else(PoisonError::into_inner) = calls;
        *self.rotation_off.lock().unwrap_or_else(PoisonError::into_inner) = off;
        self.rotation_warned.store(false, Ordering::SeqCst);
    }

    // disable: the rotation is dropped. The runner forgets the hidden inventory; no packet is sent here (disable may
    // run off the game thread)
    pub fn stop_rotation(&self) {
        if let Some(container_id) = self.reset_rotation() {
            curia::warn!("the mod was disabled while a hidden inventory was open", { container_id: i64::from(container_id) });
        }
    }

    // forgets the rotation and what the gate kept; gives the container still owed a close
    fn reset_rotation(&self) -> Option<i8> {
        let owed = self.rotation.lock().unwrap_or_else(PoisonError::into_inner).reset();
        let _ = (self.gate.take_early_open(), self.gate.take_early_close());
        owed
    }

    // whether a rotation waits for the server or runs
    pub fn rotation_active(&self) -> bool {
        self.rotation.lock().unwrap_or_else(PoisonError::into_inner).active()
    }

    // why a rotation key does nothing; None when the rotation can run. Each reason is a part that is off
    pub fn rotation_blocked(&self) -> Option<String> {
        if let Some(reason) = self.rotation_off.lock().unwrap_or_else(PoisonError::into_inner).clone() {
            return Some(reason);
        }
        if !self.moves_ready() {
            return Some("the item moves are off".to_string());
        }
        let entries = self.game_entries.lock().unwrap_or_else(PoisonError::into_inner).is_some();
        if !entries || !self.slot_reader_ready() {
            return Some("the player slots cannot be read (an entry or the slot reader is missing)".to_string());
        }
        None
    }

    // a rotation key: opens the player inventory hidden from the game (Interact), and the rotation goes on when the
    // server's open arrives (`on_container_open`) and at the next tick. Nothing happens while a screen is open (the HUD
    // must be the top screen), a rotation, a sort or a queued action runs, or a part of it is off (one warning)
    pub fn on_rotate(&self, direction: Direction, now: f64) {
        // a screen action owns the cursor and the slots until it ends
        if self.sort_running() || self.pending_action().is_some() {
            return;
        }
        if let Some(reason) = self.rotation_blocked() {
            if !self.rotation_warned.swap(true, Ordering::SeqCst) {
                curia::warn!("the row rotation is off", { reason: reason });
            }
            return;
        }
        let screen_open = !self.lock_guard().hud();
        let result = self.with_link(|engine, link| {
            let mut link = self.gate.gated(link);
            self.rotation.lock().unwrap_or_else(PoisonError::into_inner).request(
                direction,
                now,
                screen_open,
                engine,
                &mut link,
            )
        });
        match result {
            Some(RotationResult::Started) => {
                // a server that answered inside the send: its open was hidden and kept for the runner
                if let Some(container_id) = self.gate.take_early_open() {
                    let mut runner = self.rotation.lock().unwrap_or_else(PoisonError::into_inner);
                    runner.on_container_open(now, container_id, RotationRunner::INVENTORY_TYPE);
                }
            }
            Some(RotationResult::Ignored(RotationIgnored::NotSent)) | None => {
                curia::warn!("the row rotation was not started; the inventory open could not be sent");
            }
            // a key pressed over a screen, or while another action runs, is not an error
            Some(RotationResult::Ignored(_)) => {}
        }
    }

    // the body of the ContainerOpen handler detour: true when the game must not get the packet. `packet` is the
    // handler's `ContainerOpenPacket const&`. The server's open of the hidden inventory is hidden; any other open goes
    // to the game
    #[allow(unsafe_code)]
    pub fn on_container_open(&self, packet: usize) -> bool {
        // SAFETY: the handler's live ContainerOpenPacket, laid out as the mirror
        let Some(packet) = (unsafe { (packet as *const ContainerOpenPacketLayout).as_ref() }) else { return false };
        let (id, kind) = (packet.container_id, packet.container_type);
        match self.rotation.try_lock() {
            Ok(mut runner) => runner.on_container_open(self.now(), id, kind),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().on_container_open(self.now(), id, kind),
            // the runner is busy sending: only an answer inside the Interact send is ours (see `AnswerGate`)
            Err(TryLockError::WouldBlock) => self.gate.busy_open(id, kind),
        }
    }

    // the body of the ContainerClose handler detour: true when the game must not get the packet. The server's answer to
    // the rotation's close is hidden; any other close goes to the game
    #[allow(unsafe_code)]
    pub fn on_container_close(&self, packet: usize) -> bool {
        // SAFETY: the handler's live ContainerClosePacket, laid out as the mirror
        let Some(packet) = (unsafe { (packet as *const ContainerClosePacketLayout).as_ref() }) else { return false };
        let id = packet.container_id;
        match self.rotation.try_lock() {
            Ok(mut runner) => runner.on_container_close(self.now(), id),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().on_container_close(self.now(), id),
            // the runner is busy sending: only an answer inside the close send is ours (see `AnswerGate`)
            Err(TryLockError::WouldBlock) => self.gate.busy_close(id),
        }
    }

    // once per client tick while a rotation runs: the runner starts the moves, drives the engine from the player's own
    // slots and closes the hidden inventory when the engine is done or a screen opened. Only a rotation that did not
    // end quietly is logged
    fn drive_rotation(&self, now: f64) {
        let hud = self.lock_guard().hud();
        let client = self.client.load(Ordering::SeqCst) as *mut c_void;
        let entries = self.game_entries.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let players = *self.player_calls.lock().unwrap_or_else(PoisonError::into_inner);
        let reader = self.slot_reader.lock().unwrap_or_else(PoisonError::into_inner);
        let source = match (entries.as_deref(), reader.as_ref(), players) {
            // SAFETY: the client of the last ClientInstance::update, live while the game runs, used on the game thread;
            // the entries and calls were resolved for this build at enable
            #[allow(unsafe_code)]
            (Some(entries), Some(reader), Some(players)) => {
                Some(unsafe { PlayerSlots::new(entries, reader, players, client) })
            }
            _ => None,
        };
        let end = self.with_link(|engine, link| {
            let source = source.as_ref().map(|source| source as &dyn ScreenSource);
            let mut link = self.gate.gated(link);
            self.rotation.lock().unwrap_or_else(PoisonError::into_inner).tick(now, hud, source, engine, &mut link)
        });
        // the server's answer to the close came inside the send: it was hidden, and the window ends here
        if let Some(container_id) = self.gate.take_early_close() {
            self.rotation.lock().unwrap_or_else(PoisonError::into_inner).on_container_close(now, container_id);
        }
        match end {
            Some(Some(end)) => Self::log_rotation_end(end),
            Some(None) => {}
            // no packet path any more (no client): the rotation cannot close what it opened, and must not wait for it
            None => {
                if self.reset_rotation().is_some() {
                    curia::warn!("the row rotation was dropped; the client, its sender or its player is gone");
                }
                self.cancel_moves();
            }
        }
    }

    // `body` on the engine and the hidden inventory link of the client, with the client and the answers given to the
    // engine first. None, with `body` not run, when the link cannot be made (no client, no sender, no player)
    fn with_link<R>(
        &self,
        body: impl FnOnce(&mut MoveEngine<PlayerPredictor>, &mut dyn InventoryLink) -> R,
    ) -> Option<R> {
        let packets = *self.packet_calls.lock().unwrap_or_else(PoisonError::into_inner);
        let players = *self.player_calls.lock().unwrap_or_else(PoisonError::into_inner);
        let rotation = *self.rotation_calls.lock().unwrap_or_else(PoisonError::into_inner);
        let client = self.client.load(Ordering::SeqCst) as *mut c_void;
        let (packets, players, rotation) = (packets?, players?, rotation?);
        // SAFETY: the client of the last ClientInstance::update, live while the game runs, used on the game thread; the
        // calls were resolved for the running build
        #[allow(unsafe_code)]
        let mut link = unsafe { HiddenInventory::of_client(client, &packets, &players, &rotation) }?;
        let mut engine = self.lock_moves();
        // SAFETY: as above
        #[allow(unsafe_code)]
        unsafe {
            engine.predictor_mut().set_client(client)
        };
        self.drain_answers(&mut engine);
        let result = body(&mut engine, &mut link);
        self.drain_answers(&mut engine);
        Some(result)
    }

    fn log_rotation_end(end: RotationEnd) {
        match end {
            // success is silent
            RotationEnd::Done => {}
            RotationEnd::Cancelled => curia::info!("the row rotation was cancelled; a screen opened"),
            RotationEnd::Moves(outcome) => {
                let fields = (outcome.accepted as u64, outcome.total as u64);
                curia::warn!(
                    "the row rotation ended early; the next rotation reads the slots again",
                    { accepted: fields.0, total: fields.1, end: format!("{:?}", outcome.end) }
                );
            }
            RotationEnd::MoveOnItsWay => curia::info!("the row rotation waits: a move is still on its way"),
            RotationEnd::NoRead => curia::warn!("the row rotation did nothing; the player slots could not be read"),
            RotationEnd::NotStarted => curia::warn!("the row rotation did nothing; the move engine did not start"),
            RotationEnd::NoAnswer => {
                curia::warn!("the row rotation did nothing; the server did not open the inventory");
            }
        }
    }

    fn log_result(&self, action: ButtonAction, result: ActionResult) {
        match result {
            ActionResult::Done { calls } => {
                curia::info!("action done", { action: format!("{action:?}"), calls: calls as u64 });
            }
            ActionResult::Started { .. } => curia::info!("sort started", { action: format!("{action:?}") }),
            ActionResult::Ignored(reason) => {
                curia::info!("action ignored", { action: format!("{action:?}"), reason: format!("{reason:?}") });
            }
        }
    }

    fn log_sort_end(end: SortEnd) {
        let steps = end.steps as u64;
        match end.reason {
            None => curia::info!("sort done", { steps: steps }),
            // the player closed or switched the screen: not a fault
            Some(reason) if reason == FeatureRunner::SCREEN_GONE => {
                curia::info!("sort stopped", { steps: steps, reason: reason.to_string() });
            }
            Some(reason) => curia::warn!("sort stopped", { steps: steps, reason: reason.to_string() }),
        }
        if end.left_on_cursor {
            curia::warn!(
                "the cursor still holds a stack after the sort; closing the screen gives it back to the player"
            );
        }
    }

    fn warn_screen_off(&self) {
        if !self.screen_warned.swap(true, Ordering::SeqCst) {
            curia::warn!(
                "the open screen cannot be read (an entry or the slot reader is missing); the actions do nothing"
            );
        }
    }

    // the buttons' lock for the detours, which never wait: a busy lock (a re-entry from game code the paint calls,
    // which would deadlock on a non-reentrant Mutex) gives None, the click passes to the game and the frame draws no
    // buttons. One warning per process
    fn try_lock_buttons(&self) -> Option<std::sync::MutexGuard<'_, ButtonUi>> {
        match self.buttons.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => {
                if !self.busy_warned.swap(true, Ordering::SeqCst)
                    && let Some(runtime) = ModRuntime::get()
                {
                    runtime
                        .logger()
                        .warn("the buttons were busy for a click or a frame; it went to the game unchanged");
                }
                None
            }
        }
    }

    fn lock_buttons(&self) -> std::sync::MutexGuard<'_, ButtonUi> {
        self.buttons.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_runner(&self) -> std::sync::MutexGuard<'_, FeatureRunner> {
        self.runner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_moves(&self) -> std::sync::MutexGuard<'_, MoveEngine<PlayerPredictor>> {
        self.moves.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn drain_answers(&self, engine: &mut MoveEngine<PlayerPredictor>) {
        let answers = std::mem::take(&mut *self.answers.lock().unwrap_or_else(PoisonError::into_inner));
        for (id, accepted) in answers {
            engine.on_response(id, accepted);
        }
    }

    fn log_link_error(error: &LinkError) {
        let Some(runtime) = ModRuntime::get() else { return };
        let unsupported = LinkError::from(AmmStatus::Unsupported);
        match error {
            LinkError::ManagerMissing | LinkError::AbiMismatch { .. } => {
                runtime.logger().error("@alaydriem/hotbar-manager needs @amml/modmanager 0.2.x; the mod is off");
            }
            other if *other == unsupported => {
                runtime.logger().warn("@amml/modmanager is too old for @alaydriem/hotbar-manager; the mod is off");
            }
            other => {
                runtime
                    .logger()
                    .error(&format!("@amml/modmanager refused @alaydriem/hotbar-manager ({other}); the mod is off"));
            }
        }
    }
}
