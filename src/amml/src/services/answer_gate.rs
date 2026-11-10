use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::services::RotationRunner;
use crate::traits::{InventoryLink, PacketSink};

// Hides, while the rotation runner's lock is busy, only an answer the mod itself caused. A local server may answer the
// Interact or the close inside the send, while the runner's lock is held, so the ContainerOpen or ContainerClose
// handler cannot reach the runner. The gate arms a flag around the one send that can cause the answer (the Interact,
// the close; never a request send, never the whole tick) and the handler asks the gate. An open is hidden when it is
// the Inventory type inside the Interact send; a close is hidden when it names the container being closed inside the
// close send, once. Anything else, and every answer outside a send, is not hidden: the game gets it, as stratum does
// when its lock is busy. What the gate hid is kept for the runner, which takes it when its lock is free
pub struct AnswerGate {
    opening: AtomicBool,
    closing: AtomicBool,
    closing_id: AtomicI32,
    early_open: Mutex<Option<i8>>,
    early_close: Mutex<Option<i8>>,
}

impl AnswerGate {
    pub fn new() -> Self {
        Self {
            opening: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            closing_id: AtomicI32::new(0),
            early_open: Mutex::new(None),
            early_close: Mutex::new(None),
        }
    }

    // `link` with the flags armed around its open and close sends
    pub fn gated<'a>(&'a self, link: &'a mut dyn InventoryLink) -> GatedLink<'a> {
        GatedLink { gate: self, link }
    }

    // a ContainerOpen came while the runner's lock was busy: true (hide, kept for the runner) only inside the Interact
    // send, for the Inventory type
    pub fn busy_open(&self, container_id: i8, container_type: i8) -> bool {
        let ours = self.opening.load(Ordering::SeqCst) && container_type == RotationRunner::INVENTORY_TYPE;
        if ours {
            *self.early_open.lock().unwrap_or_else(PoisonError::into_inner) = Some(container_id);
        }
        ours
    }

    // a ContainerClose came while the runner's lock was busy: true (hide, kept for the runner) only inside the close
    // send, for the container being closed, once
    pub fn busy_close(&self, container_id: i8) -> bool {
        let ours =
            self.closing.load(Ordering::SeqCst) && i32::from(container_id) == self.closing_id.load(Ordering::SeqCst);
        if ours {
            self.closing.store(false, Ordering::SeqCst);
            *self.early_close.lock().unwrap_or_else(PoisonError::into_inner) = Some(container_id);
        }
        ours
    }

    // the open the gate hid, once
    pub fn take_early_open(&self) -> Option<i8> {
        self.early_open.lock().unwrap_or_else(PoisonError::into_inner).take()
    }

    // the close the gate hid, once
    pub fn take_early_close(&self) -> Option<i8> {
        self.early_close.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

impl Default for AnswerGate {
    fn default() -> Self {
        Self::new()
    }
}

// a link that arms the gate around its open and close sends
pub struct GatedLink<'a> {
    gate: &'a AnswerGate,
    link: &'a mut dyn InventoryLink,
}

impl PacketSink for GatedLink<'_> {
    fn send(&mut self, bytes: &[u8]) -> bool {
        self.link.send(bytes)
    }
}

impl InventoryLink for GatedLink<'_> {
    fn open_inventory(&mut self) -> bool {
        self.gate.opening.store(true, Ordering::SeqCst);
        let sent = self.link.open_inventory();
        self.gate.opening.store(false, Ordering::SeqCst);
        sent
    }

    fn close_inventory(&mut self, container_id: i8) -> bool {
        self.gate.closing_id.store(i32::from(container_id), Ordering::SeqCst);
        self.gate.closing.store(true, Ordering::SeqCst);
        let sent = self.link.close_inventory(container_id);
        self.gate.closing.store(false, Ordering::SeqCst);
        sent
    }
}
