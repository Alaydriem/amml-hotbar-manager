// The argument structs of the container handlers, as the game lays them out. They are built on the Rust stack, so
// no game heap is used: the collection names fit the 15-byte inline buffer of an MSVC `std::string`

// A `std::string` that holds its text inline: a 16-byte buffer, the size and the capacity (15)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct MsvcString {
    buffer: [u8; 16],
    size: u64,
    capacity: u64,
}

impl MsvcString {
    pub const INLINE_CAPACITY: usize = 15;

    // None for a text of more than 15 bytes
    pub fn inline(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() > Self::INLINE_CAPACITY {
            return None;
        }
        let mut buffer = [0u8; 16];
        buffer[..bytes.len()].copy_from_slice(bytes);
        Some(Self { buffer, size: bytes.len() as u64, capacity: Self::INLINE_CAPACITY as u64 })
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    pub fn buffer(&self) -> &[u8; 16] {
        &self.buffer
    }
}

// `SlotData`: a collection name and an index (40 bytes with the padding after the index)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlotData {
    pub name: MsvcString,
    pub index: i32,
}

// `ItemTransferAmount`: the tag (0 RequestAmount, 1 TakeType, 2 PlaceType), the value (TakeType: 0 = all, 2 = one)
// and a flag
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ItemTransferAmount {
    pub tag: i32,
    pub value: i32,
    pub flag: bool,
}

impl ItemTransferAmount {
    // the whole stack
    pub const WHOLE_STACK: Self = Self { tag: 1, value: 0, flag: false };
}
