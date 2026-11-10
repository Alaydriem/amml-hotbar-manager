use std::ffi::c_void;

use amml_game::RawObject;

use crate::errors::PacketReadError;
use crate::game::{GamePacket, PacketId};
use crate::traits::PacketSink;
use crate::types::PacketCalls;

// `void PacketSender::send(Packet&)`: this, the packet
type SendFn = unsafe extern "C" fn(*mut c_void, *mut c_void);

// The client's `PacketSender`, as `ClientInstance::getPacketSender` gives it: what the game's own screens send their
// packets through. Copied from stratum; as a `PacketSink` it sends item stack requests. Game thread only
pub struct PacketSender {
    sender: *mut c_void,
    send_slot: u64,
    calls: PacketCalls,
}

impl PacketSender {
    /// the sender of `client`; None for a null client or sender
    ///
    /// # Safety
    /// `client` must be null or the live `ClientInstance` of the build `calls` was resolved for
    #[allow(unsafe_code)]
    pub unsafe fn of_client(client: *mut c_void, calls: &PacketCalls) -> Option<Self> {
        if client.is_null() {
            return None;
        }
        // SAFETY: the caller guarantees a live client, whose vtable holds getPacketSender at the entry's slot
        let function = unsafe { RawObject::slot(client, calls.get_packet_sender_slot) }?;
        // SAFETY: the slot holds `PacketSender& ClientInstance::getPacketSender()`
        let sender = unsafe { RawObject::call_pointer(function, client) }?;
        Some(Self { sender, send_slot: calls.send_slot, calls: *calls })
    }

    // sends `packet` to the server
    pub fn send_packet(&self, packet: &GamePacket) {
        let target = packet.as_ptr();
        if target.is_null() {
            return;
        }
        // SAFETY: the client's live sender, whose vtable holds send at the entry's slot
        #[allow(unsafe_code)]
        let Some(function) = (unsafe { RawObject::slot(self.sender, self.send_slot) }) else { return };
        // SAFETY: the slot holds `void PacketSender::send(Packet&)`
        #[allow(unsafe_code)]
        let send: SendFn = unsafe { std::mem::transmute(function) };
        // SAFETY: a live packet that the wrapper holds a use of for the call
        #[allow(unsafe_code)]
        unsafe {
            send(self.sender, target)
        };
    }

    // makes a packet of `id`, fills it from `bytes` (the payload, without the packet header) and sends it. A packet
    // whose read fails is not sent. The packet and its stream are released once either way
    pub fn send_bytes(&self, id: PacketId, bytes: &[u8]) -> Result<(), PacketReadError> {
        let mut packet = GamePacket::create(&self.calls, id).ok_or(PacketReadError::NotMade)?;
        packet.read_from(bytes)?;
        self.send_packet(&packet);
        Ok(())
    }
}

impl PacketSink for PacketSender {
    fn send(&mut self, bytes: &[u8]) -> bool {
        match self.send_bytes(PacketId::ITEM_STACK_REQUEST, bytes) {
            Ok(()) => true,
            Err(error) => {
                curia::warn!("item stack request not sent", { error: error.to_string() });
                false
            }
        }
    }
}
