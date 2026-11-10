use alaydriem_hotbar_manager::traits::{MovePredictor, PacketSink};
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::ItemMove;

// a packet sink stand-in: records every request it is given; `refuse` makes each send fail
#[derive(Default)]
pub struct FakeSender {
    pub sent: Vec<Vec<u8>>,
    pub refuse: bool,
}

impl PacketSink for FakeSender {
    fn send(&mut self, bytes: &[u8]) -> bool {
        if self.refuse {
            return false;
        }
        self.sent.push(bytes.to_vec());
        true
    }
}

// a predictor stand-in: predicts every move (unless `skip`), and records what the engine asked of it
#[derive(Default)]
pub struct FakePredictor {
    pub skip: bool,
    pub predicted: Vec<ItemMove>,
    pub confirmed: Vec<ItemMove>,
    pub undone: Vec<ItemMove>,
}

impl MovePredictor for FakePredictor {
    fn predict(&mut self, mv: &ItemMove, _before: &SlotModel) -> bool {
        if self.skip {
            return false;
        }
        self.predicted.push(*mv);
        true
    }

    fn confirm(&mut self, mv: &ItemMove) {
        self.confirmed.push(*mv);
    }

    fn undo(&mut self, mv: &ItemMove) -> bool {
        self.undone.push(*mv);
        true
    }
}
