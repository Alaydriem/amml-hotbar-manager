use hotbar_manager_core::services::RotationPlanner;
use hotbar_manager_core::structs::{Direction, ScreenKind};

use crate::services::MoveEngine;
use crate::traits::{InventoryLink, MovePredictor, ScreenSource};
use crate::types::{MoveEnd, MoveOutcome};

// why a rotation key did nothing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationIgnored {
    // a screen is open: the rotation works on the HUD only
    ScreenOpen,
    // a rotation waits for the server or runs, or the move engine runs another action
    Busy,
    // the Interact packet could not be sent
    NotSent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationResult {
    // Interact(OpenInventory) was sent; the rotation goes on when the server's open arrives
    Started,
    Ignored(RotationIgnored),
}

// how a rotation ended; each end has closed the hidden inventory. Only Done is silent
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationEnd {
    // every move was accepted, or no row had an item to move
    Done,
    // a screen opened before the moves started
    Cancelled,
    // the moves ended before the last one was accepted (refused, no answer, stale, cancelled by a screen)
    Moves(MoveOutcome),
    // the player slots could not be read
    NoRead,
    // the move engine did not start
    NotStarted,
    // the server did not open the inventory in time
    NoAnswer,
    // a player slot reads net id 0: a move (of a screen action just before) still waits for its answer. Nothing is
    // planned; the next press works once the answers are in
    MoveOnItsWay,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    // Interact was sent; waiting for the server's ContainerOpen until `until`
    Opening { dir: Direction, until: f64 },
    // the server opened the inventory as `container_id` (hidden from the game); the moves start at the next tick
    Opened { dir: Direction, container_id: i8 },
    // the engine runs the moves
    Running { container_id: i8 },
}

// Rotates the four player rows through a hidden inventory open, the way stratum's stack request sends its moves: a
// rotation key sends Interact(OpenInventory); the server's ContainerOpen answer is hidden from the game; the moves of
// `RotationPlanner` go out through the move engine as one batch (one packet); whatever the engine's outcome, a ContainerClose is sent and the
// server's answer to it is hidden. If a screen opens while it runs, the engine is cancelled and the inventory closed.
// The hidden inventory is never left open. Pure: the game is reached through `InventoryLink` and `ScreenSource`
#[derive(Debug)]
pub struct RotationRunner {
    phase: Phase,
    // the server's answer to our close is hidden until `close_until`, when it names `closed_id`
    close_until: f64,
    closed_id: i32,
}

impl RotationRunner {
    // how long the server's open and close answers are waited for: stratum's ANSWER_WINDOW_SECONDS
    pub const ANSWER_WINDOW_SECONDS: f64 = 2.0;
    // SharedTypes::Legacy::ContainerType::Inventory
    pub const INVENTORY_TYPE: i8 = -1;

    pub fn new() -> Self {
        Self { phase: Phase::Idle, close_until: 0.0, closed_id: -1 }
    }

    // whether a rotation waits for the server or runs
    pub fn active(&self) -> bool {
        self.phase != Phase::Idle
    }

    // a rotation key. `screen_open` is true unless the HUD is the top screen. Nothing happens while a screen is open,
    // a rotation is in progress, or the engine runs an action. Otherwise Interact(OpenInventory) is sent
    pub fn request<P: MovePredictor>(
        &mut self,
        dir: Direction,
        now: f64,
        screen_open: bool,
        engine: &MoveEngine<P>,
        link: &mut dyn InventoryLink,
    ) -> RotationResult {
        if screen_open {
            return RotationResult::Ignored(RotationIgnored::ScreenOpen);
        }
        if self.active() || engine.busy() {
            return RotationResult::Ignored(RotationIgnored::Busy);
        }
        if !link.open_inventory() {
            return RotationResult::Ignored(RotationIgnored::NotSent);
        }
        self.phase = Phase::Opening { dir, until: now + Self::ANSWER_WINDOW_SECONDS };
        RotationResult::Started
    }

    // the server's ContainerOpen: true hides the inventory open this rotation asked for. The moves start at the next
    // tick, not here, so the game's packet handler does no more than note the answer
    pub fn on_container_open(&mut self, now: f64, container_id: i8, container_type: i8) -> bool {
        let Phase::Opening { dir, until } = self.phase else { return false };
        if now > until || container_type != Self::INVENTORY_TYPE {
            return false;
        }
        self.phase = Phase::Opened { dir, container_id };
        true
    }

    // the server's ContainerClose: true hides its answer to our close
    pub fn on_container_close(&mut self, now: f64, container_id: i8) -> bool {
        if now > self.close_until || i32::from(container_id) != self.closed_id {
            return false;
        }
        self.close_until = 0.0;
        true
    }

    // disable: drops the rotation. Gives the container the caller must still close (best effort), None when the
    // server has not opened one
    pub fn reset(&mut self) -> Option<i8> {
        let owed = match self.phase {
            Phase::Opened { container_id, .. } | Phase::Running { container_id } => Some(container_id),
            Phase::Idle | Phase::Opening { .. } => None,
        };
        self.phase = Phase::Idle;
        self.close_until = 0.0;
        owed
    }

    // once per client tick. `hud` is true while the HUD is the top screen; `source` reads the player's slots (None when
    // they cannot be read). Starts the moves once the server opened the inventory, drives the engine, and closes the
    // inventory when the engine is done, whatever the outcome, or when a screen opens. Gives the end of a rotation,
    // once
    pub fn tick<P: MovePredictor>(
        &mut self,
        now: f64,
        hud: bool,
        source: Option<&dyn ScreenSource>,
        engine: &mut MoveEngine<P>,
        link: &mut dyn InventoryLink,
    ) -> Option<RotationEnd> {
        match self.phase {
            // a batch cancelled after its send is still owed its answers: it ends here once its time is up
            Phase::Idle => {
                engine.expire(now);
                None
            }
            Phase::Opening { until, .. } => {
                if now <= until {
                    return None;
                }
                self.phase = Phase::Idle;
                Some(RotationEnd::NoAnswer)
            }
            Phase::Opened { dir, container_id } => {
                if !hud {
                    return Some(self.close(now, container_id, link, RotationEnd::Cancelled));
                }
                match Self::start(now, dir, source, engine) {
                    Ok(()) => {
                        self.phase = Phase::Running { container_id };
                        self.run(now, hud, source, engine, link, container_id)
                    }
                    Err(end) => Some(self.close(now, container_id, link, end)),
                }
            }
            Phase::Running { container_id } => self.run(now, hud, source, engine, link, container_id),
        }
    }

    // plans the rotation on a fresh read of the player slots and starts the engine
    fn start<P: MovePredictor>(
        now: f64,
        dir: Direction,
        source: Option<&dyn ScreenSource>,
        engine: &mut MoveEngine<P>,
    ) -> Result<(), RotationEnd> {
        let model = source.and_then(ScreenSource::read).ok_or(RotationEnd::NoRead)?;
        // a request built from a net id of 0 can be refused and leave the rows half rotated
        if model.player.iter().flatten().any(|item| item.net_id == 0) {
            return Err(RotationEnd::MoveOnItsWay);
        }
        let moves = RotationPlanner::plan(&model.player, dir);
        // nothing to move: a success, the caller still closes
        if moves.is_empty() {
            return Err(RotationEnd::Done);
        }
        if !engine.start_batch(now, moves, &model, ScreenKind::PlayerInventory) {
            return Err(RotationEnd::NotStarted);
        }
        Ok(())
    }

    // one step of the engine; closes and ends when it reports an end (a cancelled batch reports it while the engine
    // still waits for the server's answers) or is idle again
    fn run<P: MovePredictor>(
        &mut self,
        now: f64,
        hud: bool,
        source: Option<&dyn ScreenSource>,
        engine: &mut MoveEngine<P>,
        link: &mut dyn InventoryLink,
        container_id: i8,
    ) -> Option<RotationEnd> {
        if !hud {
            engine.cancel();
        } else if engine.busy() {
            match source.and_then(ScreenSource::read) {
                Some(live) => engine.tick(now, &live, link),
                None => engine.cancel(),
            }
        }
        let outcome = engine.take_outcome();
        if outcome.is_none() && engine.busy() {
            return None;
        }
        let end = match outcome {
            Some(outcome) if outcome.end != MoveEnd::Done => RotationEnd::Moves(outcome),
            _ => RotationEnd::Done,
        };
        Some(self.close(now, container_id, link, end))
    }

    // sends the close, opens the window for the server's answer, and ends the rotation with `end`
    fn close(&mut self, now: f64, container_id: i8, link: &mut dyn InventoryLink, end: RotationEnd) -> RotationEnd {
        // the window opens before the send: a local server may answer inside it
        self.close_until = now + Self::ANSWER_WINDOW_SECONDS;
        self.closed_id = i32::from(container_id);
        if !link.close_inventory(container_id) {
            self.close_until = 0.0;
            curia::warn!("the hidden inventory was not closed; the close packet was not sent", {
                container_id: i64::from(container_id)
            });
        }
        self.phase = Phase::Idle;
        end
    }
}

impl Default for RotationRunner {
    fn default() -> Self {
        Self::new()
    }
}
