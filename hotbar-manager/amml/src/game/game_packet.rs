use std::ffi::c_void;
use std::mem::MaybeUninit;

use amml_game::RawObject;
use amml_msvc::SharedPtr;

use crate::errors::PacketReadError;
use crate::game::{PacketId, PacketPayload, ReadOnlyStream};
use crate::types::PacketCalls;

// `std::shared_ptr<Packet> MinecraftPackets::createPacket(MinecraftPacketIds)`, static: the buffer in rcx, the id in
// edx, the buffer back in rax
type CreatePacketFn = unsafe extern "C" fn(*mut SharedPtr<c_void>, u32) -> *mut SharedPtr<c_void>;

// `std::string_view Packet::getName() const`: this, then the buffer for the view (pointer, length)
type GetNameFn = unsafe extern "C" fn(*const c_void, *mut [usize; 2]) -> *mut [usize; 2];

// `Bedrock::Result<void> Packet::read(ReadOnlyBinaryStream&)`: this, the result buffer, the stream
type ReadFn = unsafe extern "C" fn(*mut c_void, *mut u8, *mut c_void) -> *mut u8;

// A packet from the game's factory, held through the `std::shared_ptr<Packet>` the factory made, with its name
// checked. The use it holds is given back once, when it is dropped. Copied from stratum, trimmed to the request path
pub struct GamePacket {
    packet: SharedPtr<c_void>,
    calls: PacketCalls,
    id: PacketId,
}

impl GamePacket {
    // Packet's virtuals: the destructor, getId, getName
    const GET_NAME_SLOT: u64 = 2;
    // the longest packet name the check reads
    const NAME_LIMIT: usize = 256;
    // the buffer for the Result<void>: the exe's shared read body writes the error in the first 0x40 bytes and the
    // has-value flag at 0x40; the buffer is larger, so a longer result still fits
    const RESULT_BYTES: usize = 0x80;
    const RESULT_HAS_VALUE: usize = 0x40;

    // a new packet of `id` from the game's factory; None when the factory gives none or the packet's name is not
    // the name of `id` (the packet is then given back)
    #[allow(unsafe_code)]
    pub fn create(calls: &PacketCalls, id: PacketId) -> Option<Self> {
        if calls.create_packet == 0 {
            return None;
        }
        // SAFETY: the entry is the address of `MinecraftPackets::createPacket`
        let create: CreatePacketFn = unsafe { std::mem::transmute(calls.create_packet) };
        let mut out = MaybeUninit::<SharedPtr<c_void>>::zeroed();
        // SAFETY: `out` is a buffer for a shared_ptr; the factory constructs it, empty for an unknown id
        unsafe { create(out.as_mut_ptr(), id.id) };
        // SAFETY: the factory wrote a shared_ptr, or left the zeroed, empty one
        let packet = unsafe { out.assume_init() };
        let made = Self { packet, calls: *calls, id };
        let pointer = made.as_ptr();
        if pointer.is_null() {
            return None;
        }
        // SAFETY: a packet from the factory, whose vtable starts with Packet's virtuals
        let name = unsafe { Self::name_of(pointer) }.unwrap_or_default();
        if name != id.name {
            curia::warn!("the game made a packet of another type; it is not used", {
                id: id.id,
                expected: id.name,
                found: name
            });
            return None;
        }
        Some(made)
    }

    // the packet object, as `PacketSender::send` takes it
    pub fn as_ptr(&self) -> *mut c_void {
        self.packet.get().map_or(std::ptr::null_mut(), |packet| std::ptr::from_ref(packet).cast_mut())
    }

    // the packet as the layout mirror `T`, whose packet id must be the id this packet was made with (its name was
    // checked at `create`); None for any other mirror
    pub fn payload_mut<T: PacketPayload>(&mut self) -> Option<&mut T> {
        if self.id != T::PACKET {
            return None;
        }
        let packet = self.as_ptr();
        // SAFETY: the game named the packet as `T` mirrors it, and this wrapper holds a use of it
        #[allow(unsafe_code)]
        unsafe {
            packet.cast::<T>().as_mut()
        }
    }

    // fills the packet from `bytes` through its one-argument `Packet::read`, the way the network layer does. The
    // stream is destroyed once before the call returns. An error result is not destroyed: its error code and empty
    // call stack are left in the mod's buffer
    pub fn read_from(&mut self, bytes: &[u8]) -> Result<(), PacketReadError> {
        let packet = self.as_ptr();
        // SAFETY: a live packet this wrapper holds a use of; its vtable holds the read at the entry's slot
        #[allow(unsafe_code)]
        let function =
            unsafe { RawObject::slot(packet, self.calls.packet_read_slot) }.ok_or(PacketReadError::NoReadSlot)?;
        // SAFETY: the slot holds `Bedrock::Result<void> Packet::read(ReadOnlyBinaryStream&)`
        #[allow(unsafe_code)]
        let read: ReadFn = unsafe { std::mem::transmute(function) };
        let mut stream = ReadOnlyStream::new(bytes, &self.calls);
        let mut result = [0u64; Self::RESULT_BYTES / 8];
        let out = result.as_mut_ptr().cast::<u8>();
        // SAFETY: the packet, a result buffer larger than the game's Result<void>, and a live stream over `bytes`
        #[allow(unsafe_code)]
        unsafe {
            read(packet, out, stream.as_mut_ptr())
        };
        let read_to = stream.read_pointer();
        drop(stream);
        // SAFETY: the flag byte lies inside the buffer
        #[allow(unsafe_code)]
        let has_value = unsafe { out.add(Self::RESULT_HAS_VALUE).read() };
        if has_value == 1 {
            return Ok(());
        }
        Err(PacketReadError::Refused { read_to, length: bytes.len() })
    }

    /// the name a packet gives through `Packet::getName`; None for a null vtable or slot, or a name that is not text
    ///
    /// # Safety
    /// `packet` must be a live packet of the game
    #[allow(unsafe_code)]
    pub unsafe fn name_of(packet: *const c_void) -> Option<String> {
        // SAFETY: the caller guarantees a live packet, whose vtable holds getName at the slot
        let function = unsafe { RawObject::slot(packet, Self::GET_NAME_SLOT) }?;
        // SAFETY: the slot holds `std::string_view getName() const`
        let get_name: GetNameFn = unsafe { std::mem::transmute(function) };
        let mut view = [0usize; 2];
        // SAFETY: `view` is the 16-byte buffer for the string_view
        unsafe { get_name(packet, &mut view) };
        let [text, length] = view;
        if text == 0 || length > Self::NAME_LIMIT {
            return None;
        }
        // SAFETY: the view points to the packet's static name of `length` bytes
        let bytes = unsafe { std::slice::from_raw_parts(text as *const u8, length) };
        std::str::from_utf8(bytes).ok().map(str::to_string)
    }
}

impl Drop for GamePacket {
    fn drop(&mut self) {
        // SAFETY: the factory gave the pointer with one use, which this wrapper owns and gives back once
        #[allow(unsafe_code)]
        unsafe {
            self.packet.release()
        };
    }
}
