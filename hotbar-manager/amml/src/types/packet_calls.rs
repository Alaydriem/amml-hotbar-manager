use amml_sdk::SdkError;

use crate::traits::GameHost;

// The resolved game code of the packet path: the factory, the one-argument `Packet::read` slot, the stream the read
// takes, and the client's packet sender. The entry names are stratum's
#[derive(Clone, Copy, Debug)]
pub struct PacketCalls {
    // `std::shared_ptr<Packet> MinecraftPackets::createPacket(MinecraftPacketIds)`
    pub create_packet: usize,
    // vtable slot of `Bedrock::Result<void> Packet::read(ReadOnlyBinaryStream&)`
    pub packet_read_slot: u64,
    // the address of the `ReadOnlyBinaryStream` vtable
    pub stream_vtable: usize,
    // `ReadOnlyBinaryStream::~ReadOnlyBinaryStream()`, the body
    pub stream_dtor: usize,
    // vtable slot of `PacketSender& ClientInstance::getPacketSender()`
    pub get_packet_sender_slot: u64,
    // vtable slot of `void PacketSender::send(Packet&)`
    pub send_slot: u64,
}

impl PacketCalls {
    pub const CREATE_PACKET: &'static str = "MinecraftPackets.createPacket";
    pub const PACKET_READ: &'static str = "Packet.read";
    pub const STREAM_VTABLE: &'static str = "ReadOnlyBinaryStream.vtable";
    pub const STREAM_DTOR: &'static str = "ReadOnlyBinaryStream.dtor";
    pub const GET_PACKET_SENDER: &'static str = "ClientInstance.getPacketSender";
    pub const SEND: &'static str = "PacketSender.send";
    pub const ENTRIES: [&'static str; 6] = [
        Self::CREATE_PACKET,
        Self::PACKET_READ,
        Self::STREAM_VTABLE,
        Self::STREAM_DTOR,
        Self::GET_PACKET_SENDER,
        Self::SEND,
    ];

    // the calls of the running build; an error names the first entry that is missing
    pub fn resolve<H: GameHost>(host: &H) -> Result<Self, SdkError> {
        Ok(Self {
            create_packet: host.resolve(Self::CREATE_PACKET)? as usize,
            packet_read_slot: host.resolve(Self::PACKET_READ)?,
            stream_vtable: host.resolve(Self::STREAM_VTABLE)? as usize,
            stream_dtor: host.resolve(Self::STREAM_DTOR)? as usize,
            get_packet_sender_slot: host.resolve(Self::GET_PACKET_SENDER)?,
            send_slot: host.resolve(Self::SEND)?,
        })
    }
}
