use amml_game::{ContainerClosePacketLayout, ContainerOpenPacketLayout, InteractPacketLayout};

use crate::game::PacketId;

// A layout mirror of a whole game packet (the `Packet` base, its payload and `mSerializationMode`), with the packet id
// it mirrors. A `GamePacket` gives its object as this layout only when it was made with that id
pub trait PacketPayload {
    const PACKET: PacketId;
}

impl PacketPayload for InteractPacketLayout {
    const PACKET: PacketId = PacketId::INTERACT;
}

impl PacketPayload for ContainerOpenPacketLayout {
    const PACKET: PacketId = PacketId::CONTAINER_OPEN;
}

impl PacketPayload for ContainerClosePacketLayout {
    const PACKET: PacketId = PacketId::CONTAINER_CLOSE;
}
