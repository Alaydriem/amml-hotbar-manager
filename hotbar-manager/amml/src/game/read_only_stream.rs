use std::ffi::c_void;
use std::marker::PhantomData;
use std::mem::{MaybeUninit, offset_of};

use amml_game::ReadOnlyBinaryStreamLayout;

use crate::types::PacketCalls;

// `ReadOnlyBinaryStream::~ReadOnlyBinaryStream()`: rcx the stream
type StreamDtorFn = unsafe extern "C" fn(*mut c_void);

// A game `ReadOnlyBinaryStream` over bytes the mod owns, as the constructor with `copyBuffer` false makes it: the
// game's vtable, an empty owned buffer, and the view on the bytes. It is destroyed once, through the game's
// destructor, when it is dropped. Copied from stratum
pub struct ReadOnlyStream<'a> {
    layout: ReadOnlyBinaryStreamLayout,
    dtor: usize,
    bytes: PhantomData<&'a [u8]>,
}

impl<'a> ReadOnlyStream<'a> {
    // an empty MSVC std::string: 16 zero inline bytes, size 0, capacity 15
    const EMPTY_STRING: [usize; 4] = [0, 0, 0, 15];

    pub fn new(bytes: &'a [u8], calls: &PacketCalls) -> Self {
        let mut layout = MaybeUninit::<ReadOnlyBinaryStreamLayout>::zeroed();
        let base = layout.as_mut_ptr().cast::<u8>();
        // SAFETY: the writes stay inside the zeroed mirror, each at its field's offset and with its field's size
        #[allow(unsafe_code)]
        let layout = unsafe {
            base.add(offset_of!(ReadOnlyBinaryStreamLayout, vtable)).cast::<usize>().write(calls.stream_vtable);
            base.add(offset_of!(ReadOnlyBinaryStreamLayout, owned_buffer))
                .cast::<[usize; 4]>()
                .write_unaligned(Self::EMPTY_STRING);
            base.add(offset_of!(ReadOnlyBinaryStreamLayout, view)).cast::<*const u8>().write(bytes.as_ptr());
            base.add(offset_of!(ReadOnlyBinaryStreamLayout, view_len)).cast::<usize>().write(bytes.len());
            layout.assume_init()
        };
        Self { layout, dtor: calls.stream_dtor, bytes: PhantomData }
    }

    // the stream, as `Packet::read` takes it
    pub fn as_mut_ptr(&mut self) -> *mut c_void {
        std::ptr::from_mut(&mut self.layout).cast()
    }

    // how far the game read: `mReadPointer`
    pub fn read_pointer(&self) -> u64 {
        self.layout.read_pointer
    }
}

impl Drop for ReadOnlyStream<'_> {
    fn drop(&mut self) {
        if self.dtor == 0 {
            return;
        }
        // SAFETY: the entry is the address of the stream destructor
        #[allow(unsafe_code)]
        let dtor: StreamDtorFn = unsafe { std::mem::transmute(self.dtor) };
        let stream = self.as_mut_ptr();
        // SAFETY: the stream was made with the game's vtable and an empty owned buffer, and is destroyed once
        #[allow(unsafe_code)]
        unsafe {
            dtor(stream)
        };
    }
}
