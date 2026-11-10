use crate::game::SlotRef;

// Writes the payload of one item stack request the way the game's `BinaryStream` writers do, so that `Packet::read`
// of an `ItemStackRequestPacket` reads it: one request, its id, its actions, then no strings to filter and their
// origin -1. Each action starts with its index in the game's action variant, then its type. The `93 01` packet
// header is not written: the packet the factory makes is filled from the payload, then sent
pub struct RequestEncoder;

impl RequestEncoder {
    const ACTION_PLACE: u8 = 1;
    const ACTION_SWAP: u8 = 2;
    const ACTION_DROP: u8 = 3;
    const NOT_RANDOMLY: u8 = 0;
    const ORIGIN_NONE: i32 = -1;

    // exchanges the stacks of `from` and `to`; both slots hold a stack
    pub fn swap(request_id: i32, from: SlotRef, to: SlotRef) -> Vec<u8> {
        let mut out = Self::start(request_id);
        Self::write_action(&mut out, Self::ACTION_SWAP);
        Self::write_slot(&mut out, from);
        Self::write_slot(&mut out, to);
        Self::write_end(&mut out);
        out
    }

    // moves `count` items of `from` into the empty slot `to`
    pub fn place(request_id: i32, count: u8, from: SlotRef, to: SlotRef) -> Vec<u8> {
        Self::place_onto(request_id, count, from, SlotRef { net_id: 0, ..to })
    }

    // moves `count` items of `from` onto the stack in `to`, which keeps its net id (vanilla names it in the request)
    pub fn place_onto(request_id: i32, count: u8, from: SlotRef, to: SlotRef) -> Vec<u8> {
        let mut out = Self::start(request_id);
        Self::write_action(&mut out, Self::ACTION_PLACE);
        out.push(count);
        Self::write_slot(&mut out, from);
        Self::write_slot(&mut out, to);
        Self::write_end(&mut out);
        out
    }

    // throws `count` items of `from` out, not randomly, as Q and Ctrl+Q do
    pub fn drop(request_id: i32, count: u8, from: SlotRef) -> Vec<u8> {
        let mut out = Self::start(request_id);
        Self::write_action(&mut out, Self::ACTION_DROP);
        out.push(count);
        Self::write_slot(&mut out, from);
        out.push(Self::NOT_RANDOMLY);
        Self::write_end(&mut out);
        out
    }

    // `BinaryStream::writeVarInt`: the zigzag form of `value`, as an unsigned varint
    pub fn write_var_int(out: &mut Vec<u8>, value: i32) {
        Self::write_unsigned_var_int(out, ((value << 1) ^ (value >> 31)) as u32);
    }

    // `BinaryStream::writeUnsignedVarInt`: 7 bits per byte, low bits first, the high bit set on every byte but the
    // last
    pub fn write_unsigned_var_int(out: &mut Vec<u8>, mut value: u32) {
        while value >= 0x80 {
            out.push((value as u8 & 0x7F) | 0x80);
            value >>= 7;
        }
        out.push(value as u8);
    }

    // one request, its id and its one action
    fn start(request_id: i32) -> Vec<u8> {
        let mut out = Vec::with_capacity(32);
        Self::write_unsigned_var_int(&mut out, 1);
        Self::write_var_int(&mut out, request_id);
        Self::write_unsigned_var_int(&mut out, 1);
        out
    }

    // the variant index, then the type; for these actions the two are the same
    fn write_action(out: &mut Vec<u8>, kind: u8) {
        out.push(kind);
        out.push(kind);
    }

    // container, no dynamic id, slot, net id as a little-endian signed int
    fn write_slot(out: &mut Vec<u8>, slot: SlotRef) {
        out.push(slot.container);
        out.push(0);
        out.push(slot.slot);
        out.extend_from_slice(&slot.net_id.to_le_bytes());
    }

    // no strings to filter, and their origin
    fn write_end(out: &mut Vec<u8>) {
        Self::write_unsigned_var_int(out, 0);
        out.extend_from_slice(&Self::ORIGIN_NONE.to_le_bytes());
    }
}
