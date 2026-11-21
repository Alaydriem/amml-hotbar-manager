use amml_api::{InventoryMove, RequestId, RequestOutcome};
use hotbar_manager_core::services::RotationPlanner;
use hotbar_manager_core::structs::{Direction, ItemMove, SlotItem};

// why a rotation key did nothing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationIgnored {
    // a screen is open, or the top screen is not known: the rotation works on the HUD only
    ScreenOpen,
    // a rotation request is on its way, or a screen action runs
    Busy,
    // the player inventory could not be read
    NoRead,
}

impl RotationIgnored {
    // the word the log line carries
    pub fn reason(self) -> &'static str {
        match self {
            Self::ScreenOpen => "screen_open",
            Self::Busy => "busy",
            Self::NoRead => "no_read",
        }
    }
}

// how a rotation ended
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationEnd {
    // the server accepted every move
    Done { moves: u16 },
    // the request ended before the last move was accepted (refused, no answer, stale, cancelled by a screen)
    Early { outcome: RequestOutcome, accepted: u16, total: u16 },
}

// The row rotation as one request of the loader: the mod plans the moves on a read of the 36 player slots, and the loader
// opens the inventory hidden from the player, sends the moves, shows them at once on the client and puts them back for
// a move the server refuses. The runner keeps the one request that is on its way. `InventoryRequestDone` reaches every
// listener, so only the id of this request ends it. Pure: the game is reached through the runtime
#[derive(Debug, Default)]
pub struct RotationRunner {
    pending: Option<Pending>,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    id: RequestId,
    total: usize,
    tick: u64,
}

impl RotationRunner {
    // the client ticks after which a request that never ended is dropped: 30 seconds at 20 ticks a second, far over
    // the time the loader gives the server
    pub const EXPIRE_TICKS: u64 = 600;

    pub fn new() -> Self {
        Self::default()
    }

    // the moves of a rotation in the direction `dir`: swaps of two stacks and whole-stack places into an empty slot
    pub fn plan(player: &[Option<SlotItem>; 36], dir: Direction) -> Vec<InventoryMove> {
        RotationPlanner::plan(player, dir).into_iter().map(Self::request_move).collect()
    }

    fn request_move(mv: ItemMove) -> InventoryMove {
        match mv {
            ItemMove::Swap { a, b } => InventoryMove::Swap { a: a.slot, b: b.slot },
            ItemMove::Place { from, to, count } => InventoryMove::Place { from: from.slot, to: to.slot, count },
            ItemMove::Drop { from, count } => InventoryMove::Drop { from: from.slot, count },
        }
    }

    // whether a rotation request is on its way
    pub fn active(&self) -> bool {
        self.pending.is_some()
    }

    // the loader took the request `id` of `total` moves at client tick `tick`
    pub fn started(&mut self, id: RequestId, total: usize, tick: u64) {
        self.pending = Some(Pending { id, total, tick });
    }

    // `InventoryRequestDone` for `id`: the end of the rotation when `id` is the request on its way, else None (the event
    // belongs to another mod or request)
    pub fn finished(
        &mut self,
        id: RequestId,
        outcome: RequestOutcome,
        accepted: u16,
        total: u16,
    ) -> Option<RotationEnd> {
        if self.pending.is_none_or(|pending| pending.id != id) {
            return None;
        }
        self.pending = None;
        match outcome {
            RequestOutcome::Done => Some(RotationEnd::Done { moves: total }),
            outcome => Some(RotationEnd::Early { outcome, accepted, total }),
        }
    }

    // a request that has not ended `EXPIRE_TICKS` ticks after it started is dropped; gives its id and move count
    pub fn expire(&mut self, tick: u64) -> Option<(RequestId, usize)> {
        let pending = self.pending.filter(|pending| tick.saturating_sub(pending.tick) >= Self::EXPIRE_TICKS)?;
        self.pending = None;
        Some((pending.id, pending.total))
    }

    // disable or a world leave: nothing is on its way
    pub fn reset(&mut self) {
        self.pending = None;
    }
}
