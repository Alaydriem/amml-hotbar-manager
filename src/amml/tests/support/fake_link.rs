use alaydriem_hotbar_manager::traits::{InventoryLink, PacketSink};

// what the fake link was asked to send, in order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sent {
    Open,
    Request,
    Close(i8),
}

// a hidden-inventory stand-in: records every packet in the one list, so a test sees the order across the three kinds.
// `refuse_open` makes the open fail
#[derive(Default)]
pub struct FakeLink {
    pub sent: Vec<Sent>,
    // the bytes of each request payload, in order
    pub requests: Vec<Vec<u8>>,
    pub refuse_open: bool,
}

impl PacketSink for FakeLink {
    fn send(&mut self, bytes: &[u8]) -> bool {
        self.sent.push(Sent::Request);
        self.requests.push(bytes.to_vec());
        true
    }
}

impl InventoryLink for FakeLink {
    fn open_inventory(&mut self) -> bool {
        if self.refuse_open {
            return false;
        }
        self.sent.push(Sent::Open);
        true
    }

    fn close_inventory(&mut self, container_id: i8) -> bool {
        self.sent.push(Sent::Close(container_id));
        true
    }
}
