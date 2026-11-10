use hotbar_manager_core::structs::ItemMove;

// how an action of the move engine ended: `accepted` of its `total` moves were accepted by the server, in order, and
// `end` says why it stopped. Every end but Done leaves the client and the plan apart: the caller reads the slots
// again (the server's answer is the truth) before it plans anything new
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveOutcome {
    pub accepted: usize,
    pub total: usize,
    pub end: MoveEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveEnd {
    // every move was accepted
    Done,
    // the server refused move `index` (`mv`), or gave no answer in time (`timed_out`); the moves after it were
    // dropped. `undone` is true when the client prediction of that move was put back
    Rejected { mv: ItemMove, index: usize, timed_out: bool, undone: bool },
    // before move `index` was sent, a slot it uses no longer held what the plan expected; nothing was sent for it
    Stale { mv: ItemMove, index: usize },
    // move `index` could not be written or sent (no container id for the screen, a move the expected model refuses,
    // or the sink refused the bytes)
    NotSent { mv: ItemMove, index: usize },
    // `cancel` ran (the screen closed, or the mod was disabled)
    Cancelled,
}

impl MoveOutcome {
    // true when the caller must read the slots again before it trusts its model
    pub fn needs_resync(&self) -> bool {
        self.end != MoveEnd::Done
    }
}
