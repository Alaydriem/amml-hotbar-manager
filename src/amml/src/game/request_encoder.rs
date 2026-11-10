use crate::game::SlotRef;

// Writes the payload of one item stack request the way the game's `BinaryStream` writers do, so that `Packet::read`
// of an `ItemStackRequestPacket` reads it: one request, its id, its actions, then no strings to filter and their
// origin -1, once per request (`batch` writes several). Each action starts with its index in the game's action variant, then its type. The `93 01` packet
// header is not written: the packet the factory makes is filled from the payload, then sent
pub struct RequestEncoder;

// The one action of one request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestAction {
    // exchanges the stacks of `from` and `to`; both slots hold a stack
    Swap { from: SlotRef, to: SlotRef },
    // moves `count` items of `from` into the empty slot `to` (its net id is written as 0)
    Place { count: u8, from: SlotRef, to: SlotRef },
    // moves `count` items of `from` onto the stack in `to`, named with its net id
    PlaceOnto { count: u8, from: SlotRef, to: SlotRef },
    // throws `count` items of `from` out, not randomly
    Drop { count: u8, from: SlotRef },
}

impl RequestEncoder {
    const ACTION_PLACE: u8 = 1;
    const ACTION_SWAP: u8 = 2;
    const ACTION_DROP: u8 = 3;
    const NOT_RANDOMLY: u8 = 0;
    const ORIGIN_NONE: i32 = -1;

    // exchanges the stacks of `from` and `to`; both slots hold a stack
    pub fn swap(request_id: i32, from: SlotRef, to: SlotRef) -> Vec<u8> {
        Self::batch(&[(request_id, RequestAction::Swap { from, to })])
    }

    // moves `count` items of `from` into the empty slot `to`
    pub fn place(request_id: i32, count: u8, from: SlotRef, to: SlotRef) -> Vec<u8> {
        Self::batch(&[(request_id, RequestAction::Place { count, from, to })])
    }

    // moves `count` items of `from` onto the stack in `to`, which keeps its net id (vanilla names it in the request)
    pub fn place_onto(request_id: i32, count: u8, from: SlotRef, to: SlotRef) -> Vec<u8> {
        Self::batch(&[(request_id, RequestAction::PlaceOnto { count, from, to })])
    }

    // throws `count` items of `from` out, not randomly, as Q and Ctrl+Q do
    pub fn drop(request_id: i32, count: u8, from: SlotRef) -> Vec<u8> {
        Self::batch(&[(request_id, RequestAction::Drop { count, from })])
    }

    // several requests in one payload, in order: the count, then each request with its id, its one action, no
    // strings to filter and their origin. The server answers each by its id
    pub fn batch(requests: &[(i32, RequestAction)]) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + requests.len() * 28);
        Self::write_unsigned_var_int(&mut out, requests.len() as u32);
        for (request_id, action) in requests {
            Self::write_var_int(&mut out, *request_id);
            Self::write_unsigned_var_int(&mut out, 1);
            Self::write_request_action(&mut out, *action);
            Self::write_end(&mut out);
        }
        out
    }

    fn write_request_action(out: &mut Vec<u8>, action: RequestAction) {
        match action {
            RequestAction::Swap { from, to } => {
                Self::write_action(out, Self::ACTION_SWAP);
                Self::write_slot(out, from);
                Self::write_slot(out, to);
            }
            RequestAction::Place { count, from, to } => {
                Self::write_place(out, count, from, SlotRef { net_id: 0, ..to });
            }
            RequestAction::PlaceOnto { count, from, to } => Self::write_place(out, count, from, to),
            RequestAction::Drop { count, from } => {
                Self::write_action(out, Self::ACTION_DROP);
                out.push(count);
                Self::write_slot(out, from);
                out.push(Self::NOT_RANDOMLY);
            }
        }
    }

    fn write_place(out: &mut Vec<u8>, count: u8, from: SlotRef, to: SlotRef) {
        Self::write_action(out, Self::ACTION_PLACE);
        out.push(count);
        Self::write_slot(out, from);
        Self::write_slot(out, to);
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
