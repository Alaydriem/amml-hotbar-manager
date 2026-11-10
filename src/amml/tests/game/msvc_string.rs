use std::mem::{offset_of, size_of};

use alaydriem_hotbar_manager::game::{ItemTransferAmount, MsvcString, SlotData};

#[test]
fn msvc_string_is_32_bytes_and_inline_up_to_15() {
    assert_eq!(size_of::<MsvcString>(), 32);
    let name = MsvcString::inline("container_items").unwrap();
    assert_eq!(name.size(), 15);
    assert_eq!(&name.buffer()[..15], b"container_items");
    assert_eq!(name.buffer()[15], 0);
    assert_eq!(name.capacity(), 15);
    assert!(MsvcString::inline("container_items!").is_none());
    let short = MsvcString::inline("hotbar_items").unwrap();
    assert_eq!(short.size(), 12);
    assert_eq!(short.buffer()[12], 0);
    assert_eq!(short.capacity(), 15);
}

#[test]
fn slot_data_is_40_bytes_and_amount_is_12() {
    assert_eq!(size_of::<SlotData>(), 40);
    assert_eq!(size_of::<ItemTransferAmount>(), 12);
    let whole = ItemTransferAmount::WHOLE_STACK;
    assert_eq!((whole.tag, whole.value, whole.flag), (1, 0, false));
}

#[test]
fn the_slot_data_and_amount_fields_sit_at_their_game_offsets() {
    assert_eq!(offset_of!(SlotData, name), 0);
    assert_eq!(offset_of!(SlotData, index), 32);
    assert_eq!(offset_of!(ItemTransferAmount, tag), 0);
    assert_eq!(offset_of!(ItemTransferAmount, value), 4);
    assert_eq!(offset_of!(ItemTransferAmount, flag), 8);
}

#[test]
fn the_bytes_of_a_string_are_text_then_size_then_capacity() {
    let name = MsvcString::inline("hotbar_items").unwrap();
    // SAFETY: a repr(C) value of 32 initialised bytes (no padding)
    let bytes: [u8; 32] = unsafe { std::mem::transmute(name) };
    assert_eq!(&bytes[..12], b"hotbar_items");
    assert_eq!(&bytes[12..16], &[0; 4]);
    assert_eq!(u64::from_le_bytes(bytes[16..24].try_into().unwrap()), 12);
    assert_eq!(u64::from_le_bytes(bytes[24..32].try_into().unwrap()), 15);
}
