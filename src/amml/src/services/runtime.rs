use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, TryLockError};

use amm_client::link::LinkState;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amml_api::amml_abi::{InventoryRequestDone, ScreenChanged, ScreenRender, TexturesReloaded, UiPointer};
use amml_api::{
    Cancel, Client, InventoryRequestDoneExt, Logger, RequestOptions, RequestOutcome, ScreenChangedExt, ScreenId,
    ScreenRenderExt, TextureReload, Textures, TexturesReloadedExt, UiPointerExt,
};
use hotbar_manager_core::services::SortTick;
use hotbar_manager_core::structs::{ButtonAction, Direction};

use crate::services::{
    ActionResult, ButtonUi, ContainerActions, ContainerSource, FeatureRunner, HotbarLink, Ignored, KindMap,
    RotationEnd, RotationIgnored, RotationRunner, ScreenGuard, SlotConverter, SortEnd, Verdict,
};
use crate::traits::{ScreenActions, ScreenSource};
use crate::types::ClickRoute;

// What the listeners share: the manager link, the buttons, the screen actions and the row rotation. The screen actions
// (sort, dumps, drops) call the container handlers of the loader inside the `ScreenRender` listener; the rotation is
// one request of the loader (`Inventory::request_moves`). Every listener runs on the game thread, one at a time; a
// lock that is busy there means a listener was re-entered, so that call is skipped and one warning is logged
pub struct HotbarRuntime {
    log: Logger,
    link: Mutex<Option<HotbarLink>>,
    // the mod buttons: drawn by the ScreenRender listener, clicked through the UiPointer listener
    buttons: Mutex<ButtonUi>,
    // the feature layer: the steps of an action, and the sort that runs
    runner: Mutex<FeatureRunner>,
    // the button action that waits for the next frame of the open screen: the screen is read only inside its own
    // render call, where it is live
    pending: Mutex<Option<ButtonAction>>,
    // the identity of the open screen that the queued or running action is bound to
    guard: Mutex<ScreenGuard>,
    rotation: Mutex<RotationRunner>,
    // the client ticks so far, and 1 + the tick of the last sort step (0: none). Each container screen renders a second
    // view of the same screen in every frame, and a tick has several frames; a sort takes at most one step per tick,
    // so the next step always reads the screen after the game has handled the last one
    ticks: AtomicU64,
    sort_step_tick: AtomicU64,
    linked_logged: AtomicBool,
    busy_warned: AtomicBool,
}

impl HotbarRuntime {
    // the keybind actions of the descriptor, in the order it declares them: rotate_up, rotate_down, sort
    pub const ROTATE_UP_KEYBIND: u16 = 0;
    pub const ROTATE_DOWN_KEYBIND: u16 = 1;
    pub const SORT_KEYBIND: u16 = 2;

    pub fn new(log: Logger, textures: Textures) -> Self {
        Self {
            log,
            link: Mutex::new(None),
            buttons: Mutex::new(ButtonUi::new(textures, log)),
            runner: Mutex::new(FeatureRunner::new()),
            pending: Mutex::new(None),
            guard: Mutex::new(ScreenGuard::new()),
            rotation: Mutex::new(RotationRunner::new()),
            ticks: AtomicU64::new(0),
            sort_step_tick: AtomicU64::new(0),
            linked_logged: AtomicBool::new(false),
            busy_warned: AtomicBool::new(false),
        }
    }

    // enable: the keys work with this link until `detach`
    pub fn attach(&self, link: HotbarLink) {
        *self.link.lock().unwrap_or_else(PoisonError::into_inner) = Some(link);
    }

    // disable: the mod leaves the manager; nothing waits, no sort runs, nothing is kept of the screen
    pub fn detach(&self) {
        drop(self.link.lock().unwrap_or_else(PoisonError::into_inner).take());
        self.linked_logged.store(false, Ordering::SeqCst);
        self.reset_screen();
        self.rotation.lock().unwrap_or_else(PoisonError::into_inner).reset();
        self.buttons.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }

    // once per client tick on the game thread: reads the top screen, then the manager's events (the keybinds), and drops
    // a rotation request that never ended. An action that outlives its screen is dropped here
    pub fn on_tick(&self, client: Option<&Client<'_>>) {
        let tick = self.ticks.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        if let Some(screen) = client.and_then(Client::top_screen)
            && self.lock_guard().top_changed(screen) == Verdict::Cancel
        {
            self.abort_screen();
        }
        let events = {
            let mut slot = self.link.lock().unwrap_or_else(PoisonError::into_inner);
            slot.as_mut()
                .map(|link| {
                    if link.state() == LinkState::Connected && !self.linked_logged.swap(true, Ordering::SeqCst) {
                        self.log.info("hotbar linked", &[]);
                    }
                    link.drain()
                })
                .unwrap_or_default()
        };
        for event in &events {
            self.on_event(event, client);
        }
        let expired = self.rotation.lock().unwrap_or_else(PoisonError::into_inner).expire(tick);
        if let Some((id, total)) = expired {
            self.log.warn(
                "the row rotation request never ended; it is dropped",
                &[("id", &id.0.to_string()), ("moves", &total.to_string())],
            );
        }
    }

    // one event of the manager: a keybind, pressed (not a repeat or a release). The sort keybind runs the sort, the
    // rotation keybinds rotate the rows
    pub fn on_event(&self, event: &ModEvent, client: Option<&Client<'_>>) {
        let ModEvent::Keybind { action, phase: KeybindPhase::Pressed } = event else { return };
        match *action {
            Self::SORT_KEYBIND => self.on_action(ButtonAction::Sort),
            Self::ROTATE_UP_KEYBIND => self.on_rotate(Direction::Up, client),
            Self::ROTATE_DOWN_KEYBIND => self.on_rotate(Direction::Down, client),
            _ => {}
        }
    }

    // `TexturesReloaded`: the button textures are read again at the next paint
    pub fn on_textures_reloaded(&self, event: &TexturesReloaded) {
        let reason = event.reason();
        let held = self.buttons.lock().unwrap_or_else(PoisonError::into_inner).textures_reloaded(reason);
        let name = match reason {
            TextureReload::ResourcePack => "resource_pack",
            TextureReload::DeviceReset => "device_reset",
            TextureReload::Other => "other",
        };
        self.log.info("textures reloaded", &[("reason", name), ("held", &held.to_string())]);
    }

    // `ScreenChanged`: a close or a switch drops the action that was bound to the screen before
    pub fn on_screen_changed(&self, event: &ScreenChanged) {
        self.log.info("screen changed", &[("screen", &format!("{:?}", event.screen()))]);
        if self.lock_guard().screen_changed(ScreenChangedExt::new(event), event.screen()) == Verdict::Cancel {
            self.abort_screen();
        }
    }

    // `ScreenRender`, after the game drew the view: the buttons go on top. Inside this call the view and its container
    // screen are live, so this is where the screen is read and the container handlers are called: the action that waits
    // runs, and a running sort takes its next step
    pub fn on_screen_render(&self, event: &ScreenRender) {
        let Some(view) = event.view() else { return };
        if let Some(draw) = event.draw()
            && let Some(mut buttons) = self.try_lock(&self.buttons, "the buttons")
        {
            buttons.on_render(event.view_id().0, &view, &draw);
        }
        if self.pending_action().is_none() && !self.sort_running() {
            return;
        }
        // the action works with or without drawn buttons (Pocket): the kind comes from the view
        let kind = KindMap::kind(view.kind());
        if kind == hotbar_manager_core::structs::ScreenKind::Other {
            return;
        }
        let container = view.container_screen();
        let source = container.as_ref().map(|container| ContainerSource::new(container, kind));
        let mut actions = container.as_ref().map(ContainerActions::new);
        self.run_frame(
            source.as_ref().map(|source| source as &dyn ScreenSource),
            kind,
            Some(event.screen()),
            actions.as_mut().map(|actions| actions as &mut dyn ScreenActions),
        );
    }

    // `UiPointer`: a button of the mod takes the event, so the game does not get it. A release inside the pressed button
    // runs its action, after the buttons' lock is released
    pub fn on_pointer(&self, event: &mut UiPointer) {
        let position = event.position();
        let route = {
            let Some(mut buttons) = self.try_lock(&self.buttons, "the buttons") else { return };
            buttons.route(
                event.view().0,
                usize::from(event.pointer()),
                event.pressed(),
                (position.x, position.y),
                event.input_mode(),
            )
        };
        if route != ClickRoute::Pass {
            event.cancel();
        }
        if let Some(action) = ButtonUi::fired(route) {
            self.log.info("pointer taken", &[("action", &format!("{action:?}"))]);
            self.on_action(action);
        }
    }

    // `InventoryRequestDone`: the end of the row rotation, when the request is ours
    pub fn on_request_done(&self, event: &InventoryRequestDone) {
        let end = self.rotation.lock().unwrap_or_else(PoisonError::into_inner).finished(
            event.id(),
            event.outcome(),
            event.accepted(),
            event.total(),
        );
        if let Some(end) = end {
            self.log_rotation_end(end);
        }
    }

    // one frame of the open screen, inside the `ScreenRender` call of one of its views. `source` and `actions` are None
    // when the view has no container screen. A frame of another screen than the action's drops it. The action that
    // waits runs: a dump or a drop calls all its steps now, a sort starts. A running sort takes one step, at most one
    // per client tick (the first one in the frame it starts)
    pub fn run_frame(
        &self,
        source: Option<&dyn ScreenSource>,
        kind: hotbar_manager_core::structs::ScreenKind,
        screen: Option<ScreenId>,
        mut actions: Option<&mut dyn ScreenActions>,
    ) {
        if self.pending_action().is_none() && !self.sort_running() {
            return;
        }
        // the action belongs to the screen of its first frame; a frame of another screen is another screen
        if let Some(screen) = screen
            && self.lock_guard().frame(screen) == Verdict::Cancel
        {
            self.abort_screen();
            return;
        }
        let action = self.pending.lock().unwrap_or_else(PoisonError::into_inner).take();
        let Some(mut runner) = self.try_lock(&self.runner, "the screen actions") else { return };
        if let Some(action) = action {
            let result = match (source, actions.as_deref_mut()) {
                (Some(source), Some(actions)) => runner.start(action, kind, source, actions),
                _ => ActionResult::Ignored(Ignored::ActionsOff),
            };
            if !matches!(result, ActionResult::Started { .. }) {
                self.lock_guard().release();
            }
            self.log_result(action, result);
        }
        if !runner.sorting() {
            return;
        }
        let (Some(source), Some(actions)) = (source, actions) else {
            let ended = runner.stop_sort(FeatureRunner::NO_HANDLERS);
            self.lock_guard().release();
            if let Some(end) = ended {
                self.log_sort_end(end);
            }
            return;
        };
        // one step per tick: the second view of the same screen, and the later frames of this tick, wait
        let tick = self.ticks.load(Ordering::SeqCst).wrapping_add(1);
        if self.sort_step_tick.swap(tick, Ordering::SeqCst) == tick {
            return;
        }
        if let Some(SortTick::Done | SortTick::Stuck) = runner.step_sort(source, actions) {
            self.lock_guard().release();
            if let Some(end) = runner.take_sort_end() {
                self.log_sort_end(end);
            }
        }
    }

    // the one place a button action or a keybind arrives. The screen is read and its handlers called inside its own
    // render call, so the action waits for the next frame of the open screen (`run_frame`). An action while no container
    // screen is on top, an action already waiting, a running sort or a running rotation is ignored. A screen that
    // closes drops the waiting action
    pub fn on_action(&self, action: ButtonAction) {
        if !self.lock_guard().container_open() {
            self.log_result(action, ActionResult::Ignored(Ignored::NoScreen));
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

    // a rotation key: reads the 36 player slots, plans the moves and gives them to the loader as one request. Nothing
    // happens while a screen is open (the HUD must be the top screen), a rotation, a sort or a queued action runs, or
    // the inventory cannot be read
    pub fn on_rotate(&self, direction: Direction, client: Option<&Client<'_>>) {
        let busy = self.sort_running() || self.pending_action().is_some() || self.rotation_active();
        let ignored = if !self.lock_guard().hud() {
            Some(RotationIgnored::ScreenOpen)
        } else if busy {
            Some(RotationIgnored::Busy)
        } else {
            None
        };
        if let Some(reason) = ignored {
            self.log.info("rotation ignored", &[("reason", reason.reason())]);
            return;
        }
        let inventory = client.and_then(Client::local_player).and_then(|player| player.inventory());
        let player =
            inventory.as_ref().and_then(|inventory| inventory.slots()).and_then(|slots| SlotConverter::player(&slots));
        let (Some(inventory), Some(player)) = (inventory, player) else {
            self.log.info("rotation ignored", &[("reason", RotationIgnored::NoRead.reason())]);
            return;
        };
        let moves = RotationRunner::plan(&player, direction);
        if moves.is_empty() {
            self.log_rotation_end(RotationEnd::Done { moves: 0 });
            return;
        }
        match inventory.request_moves(&moves, RequestOptions { hidden_open: true, predict: true }) {
            Ok(id) => {
                let tick = self.ticks.load(Ordering::SeqCst);
                self.rotation.lock().unwrap_or_else(PoisonError::into_inner).started(id, moves.len(), tick);
            }
            Err(error) => {
                self.log.warn("the row rotation was not started", &[("error", &format!("{error:?}"))]);
            }
        }
    }

    // the action that waits for the next frame
    pub fn pending_action(&self) -> Option<ButtonAction> {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // whether a sort runs; a busy lock counts as running
    pub fn sort_running(&self) -> bool {
        match self.runner.try_lock() {
            Ok(runner) => runner.sorting(),
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner().sorting(),
            Err(TryLockError::WouldBlock) => true,
        }
    }

    // whether a rotation request is on its way
    pub fn rotation_active(&self) -> bool {
        self.rotation.lock().unwrap_or_else(PoisonError::into_inner).active()
    }

    // whether the buttons drew on `view` in their last frame
    pub fn buttons_drawn(&self, view: u64) -> bool {
        self.buttons.lock().unwrap_or_else(PoisonError::into_inner).layout(view).is_some()
    }

    // the screen the action belongs to is gone: the sort stops and nothing waits; no call reaches its screen
    fn abort_screen(&self) {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let ended = self.lock_runner().stop_sort(FeatureRunner::SCREEN_GONE);
        self.lock_guard().release();
        // only a sort that ran logs its end: an idle close logs nothing
        if let Some(end) = ended {
            self.log_sort_end(end);
        }
    }

    // disable: nothing waits, no sort runs, nothing is kept of the screen
    fn reset_screen(&self) {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = None;
        let mut runner = self.lock_runner();
        let _ = runner.stop_sort(FeatureRunner::SCREEN_GONE);
        let _ = runner.take_sort_end();
        drop(runner);
        *self.lock_guard() = ScreenGuard::new();
    }

    fn lock_guard(&self) -> MutexGuard<'_, ScreenGuard> {
        self.guard.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_runner(&self) -> MutexGuard<'_, FeatureRunner> {
        self.runner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    // a lock for a listener, which never waits: a busy lock (a listener re-entered from game code the mod called, which
    // would deadlock on a non-reentrant Mutex) gives None and the call is skipped. One warning per process
    fn try_lock<'a, T>(&self, mutex: &'a Mutex<T>, what: &str) -> Option<MutexGuard<'a, T>> {
        match mutex.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => {
                if !self.busy_warned.swap(true, Ordering::SeqCst) {
                    self.log.warn("a listener was re-entered; its call is skipped", &[("part", what)]);
                }
                None
            }
        }
    }

    fn log_rotation_end(&self, end: RotationEnd) {
        match end {
            RotationEnd::Done { moves } => self.log.info("rotation done", &[("moves", &moves.to_string())]),
            // a screen that opens during the request ends it; the server still does the moves it got
            RotationEnd::Early { outcome: RequestOutcome::Cancelled, accepted, total } => self.log.info(
                "the row rotation was cancelled; a screen opened or the world was left",
                &[("accepted", &accepted.to_string()), ("total", &total.to_string())],
            ),
            RotationEnd::Early { outcome, accepted, total } => self.log.warn(
                "the row rotation ended early; the next rotation reads the slots again",
                &[
                    ("outcome", &format!("{outcome:?}")),
                    ("accepted", &accepted.to_string()),
                    ("total", &total.to_string()),
                ],
            ),
        }
    }

    fn log_result(&self, action: ButtonAction, result: ActionResult) {
        let action = format!("{action:?}");
        match result {
            ActionResult::Done { calls } => {
                self.log.info("action done", &[("action", &action), ("calls", &calls.to_string())]);
            }
            ActionResult::Started { .. } => self.log.info("sort started", &[("action", &action)]),
            ActionResult::Ignored(reason) => {
                self.log.info("action ignored", &[("action", &action), ("reason", &format!("{reason:?}"))]);
            }
        }
    }

    fn log_sort_end(&self, end: SortEnd) {
        let steps = end.steps.to_string();
        match end.reason {
            None => self.log.info("sort done", &[("steps", &steps)]),
            // the player closed or switched the screen: not a fault
            Some(reason) if reason == FeatureRunner::SCREEN_GONE => {
                self.log.info("sort stopped", &[("steps", &steps), ("reason", reason)]);
            }
            Some(reason) => self.log.warn("sort stopped", &[("steps", &steps), ("reason", reason)]),
        }
        if end.left_on_cursor {
            self.log.warn(
                "the cursor still holds a stack after the sort; closing the screen gives it back to the player",
                &[],
            );
        }
    }
}
