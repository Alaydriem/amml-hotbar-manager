use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::ItemMove;

// The client prediction of the move engine: what the client shows of a move before the server answers. The engine
// calls `predict` right after a request is sent, then `confirm` when the server accepts it, or `undo` when the server
// refuses it, no answer comes in time, or the action is cancelled. One request is in flight at a time
pub trait MovePredictor {
    // `before` is the engine's expected model before the move; true when the client now shows the move
    fn predict(&mut self, mv: &ItemMove, before: &SlotModel) -> bool;

    fn confirm(&mut self, mv: &ItemMove);

    // puts back what `predict` changed; true when something was put back
    fn undo(&mut self, mv: &ItemMove) -> bool;
}

// predicts nothing: every move waits for the server's answer
#[derive(Debug, Default, Clone, Copy)]
pub struct NoPrediction;

impl MovePredictor for NoPrediction {
    fn predict(&mut self, _mv: &ItemMove, _before: &SlotModel) -> bool {
        false
    }

    fn confirm(&mut self, _mv: &ItemMove) {}

    fn undo(&mut self, _mv: &ItemMove) -> bool {
        false
    }
}
