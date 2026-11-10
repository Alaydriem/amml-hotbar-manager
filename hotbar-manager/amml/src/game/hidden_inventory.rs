use std::ffi::c_void;

use amml_game::{ContainerClosePacketLayout, InteractPacketLayout, RawObject};
use amml_msvc::MemberCall;

use crate::game::{GamePacket, PacketId, PacketSender};
use crate::traits::{InventoryLink, PacketSink};
use crate::types::{PacketCalls, PlayerCalls, RotationCalls};

// The packets of a hidden inventory open, as stratum's stack request sends them: Interact(OpenInventory) for the local
// player, as the game's own inventory key does, and the client's ContainerClose (type None) for the inventory the
// server opened. As a `PacketSink` it sends the item stack requests between them. Built for one tick; it keeps
// nothing. Game thread only
pub struct HiddenInventory {
    sender: PacketSender,
    packets: PacketCalls,
    runtime_id: u64,
}

impl HiddenInventory {
    // InteractPacketPayload::Action::OpenInventory
    const OPEN_INVENTORY: u8 = 6;
    // SharedTypes::Legacy::ContainerType::None
    const NO_TYPE: i8 = -9;

    /// the link of `client`'s local player; None for a null client, sender, player, or a runtime id call that is missing
    ///
    /// # Safety
    /// `client` must be null or the live `ClientInstance` of the build the calls were resolved for, on the game thread
    #[allow(unsafe_code)]
    pub unsafe fn of_client(
        client: *mut c_void,
        packets: &PacketCalls,
        players: &PlayerCalls,
        rotation: &RotationCalls,
    ) -> Option<Self> {
        // SAFETY: the caller guarantees a live client
        let sender = unsafe { PacketSender::of_client(client, packets) }?;
        if rotation.get_runtime_id == 0 || client.is_null() {
            return None;
        }
        // SAFETY: the live client, whose vtable holds getLocalPlayer at the entry's slot
        let function = unsafe { RawObject::slot(client, players.local_player_slot) }?;
        // SAFETY: the slot holds `LocalPlayer* ClientInstance::getLocalPlayer() const`
        let player = unsafe { RawObject::call_pointer(function, client) }?;
        // SAFETY: the entry is the address of getRuntimeID, a member function that returns an 8-byte class; the player
        // is live
        let runtime_id = unsafe { MemberCall::call_sret0::<u64>(rotation.get_runtime_id as *const c_void, player) };
        Some(Self { sender, packets: *packets, runtime_id })
    }
}

impl PacketSink for HiddenInventory {
    fn send(&mut self, bytes: &[u8]) -> bool {
        self.sender.send(bytes)
    }
}

impl InventoryLink for HiddenInventory {
    fn open_inventory(&mut self) -> bool {
        let Some(mut packet) = GamePacket::create(&self.packets, PacketId::INTERACT) else { return false };
        let Some(interact) = packet.payload_mut::<InteractPacketLayout>() else { return false };
        interact.action = Self::OPEN_INVENTORY;
        interact.target_id = self.runtime_id;
        self.sender.send_packet(&packet);
        true
    }

    fn close_inventory(&mut self, container_id: i8) -> bool {
        let Some(mut packet) = GamePacket::create(&self.packets, PacketId::CONTAINER_CLOSE) else { return false };
        let Some(close) = packet.payload_mut::<ContainerClosePacketLayout>() else { return false };
        close.container_id = container_id;
        close.container_type = Self::NO_TYPE;
        close.server_initiated_close = false;
        self.sender.send_packet(&packet);
        true
    }
}
