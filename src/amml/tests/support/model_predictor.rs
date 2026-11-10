use std::collections::VecDeque;

use alaydriem_hotbar_manager::game::PlayerPredictor;
use alaydriem_hotbar_manager::traits::MovePredictor;
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::ItemMove;

// a predictor that keeps a "client view" of the player slots by the rules of `PlayerPredictor`: a move the slot swap
// can show is swapped at once and queued with the net id moved into the second slot; `confirm` drops the oldest;
// `undo` takes the newest and swaps back only while the second slot still holds that net id. A test compares `view`
// with what the server did
pub struct ModelPredictor {
    pub view: SlotModel,
    done: VecDeque<(u8, u8, i32)>,
}

impl ModelPredictor {
    pub fn new(view: SlotModel) -> Self {
        Self { view, done: VecDeque::new() }
    }

    fn net_id(&self, slot: u8) -> Option<i32> {
        self.view.player[usize::from(slot)].as_ref().map(|item| item.net_id)
    }
}

impl MovePredictor for ModelPredictor {
    fn predict(&mut self, mv: &ItemMove, before: &SlotModel) -> bool {
        let Some((a, b)) = PlayerPredictor::swap_pair(mv, before) else { return false };
        let Some(moved) = self.net_id(a) else { return false };
        self.view.player.swap(usize::from(a), usize::from(b));
        self.done.push_back((a, b, moved));
        true
    }

    fn confirm(&mut self, _mv: &ItemMove) {
        self.done.pop_front();
    }

    fn undo(&mut self, _mv: &ItemMove) -> bool {
        let Some((a, b, moved)) = self.done.pop_back() else { return false };
        if self.net_id(b) != Some(moved) {
            return false;
        }
        self.view.player.swap(usize::from(a), usize::from(b));
        true
    }
}
