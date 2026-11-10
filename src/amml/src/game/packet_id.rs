// A `MinecraftPacketIds` value and the name the game's packet of that id gives through `Packet::getName`. The ids are
// network protocol numbers, which the game appends to and does not renumber
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PacketId {
    pub id: u32,
    pub name: &'static str,
}

impl PacketId {
    pub const INTERACT: Self = Self { id: 33, name: "InteractPacket" };
    pub const CONTAINER_OPEN: Self = Self { id: 46, name: "ContainerOpenPacket" };
    pub const CONTAINER_CLOSE: Self = Self { id: 47, name: "ContainerClosePacket" };
    pub const ITEM_STACK_REQUEST: Self = Self { id: 147, name: "ItemStackRequestPacket" };
    pub const ITEM_STACK_RESPONSE: Self = Self { id: 148, name: "ItemStackResponsePacket" };
}
