use alaydriem_hotbar_manager::game::{RequestAction, RequestEncoder, SlotRef};

// The vanilla captures of docs/findings/container-ids/report.md section 3, without the `93 01` packet header: the
// encoder writes the payload only, the way stratum's does, and the header is added when the packet is sent. Each test
// uses the request id, count, slots and net ids of its capture.
fn payload(hex: &str) -> Vec<u8> {
    let bytes: Vec<u8> = hex.split_whitespace().map(|byte| u8::from_str_radix(byte, 16).unwrap()).collect();
    assert_eq!(&bytes[..2], &[0x93, 0x01], "the capture starts with the ItemStackRequest header");
    bytes[2..].to_vec()
}

fn slot(container: u8, slot: u8, net_id: i32) -> SlotRef {
    SlotRef { container, slot, net_id }
}

#[test]
fn place_bytes_match_vanilla() {
    // #70: chest 6 (2 gold, net 5) to empty hotbar 4, request -33
    let expected = payload("93 01 01 41 01 01 01 02 07 00 06 05 00 00 00 1c 00 04 00 00 00 00 00 ff ff ff ff");

    assert_eq!(RequestEncoder::place(-33, 2, slot(7, 6, 5), slot(28, 4, 0)), expected);
}

#[test]
fn place_bytes_match_vanilla_with_a_two_byte_request_id() {
    // #201: main 10 (5 items, net 10) to empty hotbar 7, request -129
    let expected = payload("93 01 01 81 02 01 01 01 05 1d 00 0a 0a 00 00 00 1c 00 07 00 00 00 00 00 ff ff ff ff");

    assert_eq!(RequestEncoder::place(-129, 5, slot(29, 10, 10), slot(28, 7, 0)), expected);
}

#[test]
fn place_onto_bytes_match_vanilla() {
    // #76: chest 8 (7 iron, net 7) onto hotbar 5 (30 iron, net 13), request -39; the target keeps its net id
    let expected = payload("93 01 01 4d 01 01 01 07 07 00 08 07 00 00 00 1c 00 05 0d 00 00 00 00 ff ff ff ff");

    assert_eq!(RequestEncoder::place_onto(-39, 7, slot(7, 8, 7), slot(28, 5, 13)), expected);
}

#[test]
fn swap_bytes_match_vanilla() {
    // #72: chest 7 (net 6) with hotbar 0 (net 13), request -35
    let expected = payload("93 01 01 45 01 02 02 07 00 07 06 00 00 00 1c 00 00 0d 00 00 00 00 ff ff ff ff");

    assert_eq!(RequestEncoder::swap(-35, slot(7, 7, 6), slot(28, 0, 13)), expected);
}

#[test]
fn drop_bytes_match_vanilla() {
    // #63: the whole stack of main slot 9 (16 gold, net 15), request -27
    let expected = payload("93 01 01 35 01 03 03 10 1d 00 09 0f 00 00 00 00 00 ff ff ff ff");

    assert_eq!(RequestEncoder::drop(-27, 16, slot(29, 9, 15)), expected);
}

#[test]
fn drop_bytes_match_vanilla_from_the_cursor() {
    // #57: 64 diamonds on the cursor (net 11), request -23
    let expected = payload("93 01 01 2d 01 03 03 40 3b 00 00 0b 00 00 00 00 00 ff ff ff ff");

    assert_eq!(RequestEncoder::drop(-23, 64, slot(SlotRef::CURSOR, 0, 11)), expected);
}

#[test]
fn a_batch_of_one_matches_the_single_request() {
    let (a, b) = (slot(28, 0, 5), slot(29, 9, 7));

    assert_eq!(
        RequestEncoder::batch(&[(-100001, RequestAction::Swap { from: a, to: b })]),
        RequestEncoder::swap(-100001, a, b)
    );
}

#[test]
fn a_batch_of_two_writes_the_count_then_each_request() {
    let (a, b, c) = (slot(28, 0, 5), slot(29, 9, 7), slot(29, 18, 0));
    let first = RequestEncoder::swap(-100001, a, b);
    let second = RequestEncoder::place(-100003, 4, b, c);
    // each single payload starts with its request count, 1, in one byte
    let mut expected = vec![2u8];
    expected.extend_from_slice(&first[1..]);
    expected.extend_from_slice(&second[1..]);

    let batch = RequestEncoder::batch(&[
        (-100001, RequestAction::Swap { from: a, to: b }),
        (-100003, RequestAction::Place { count: 4, from: b, to: c }),
    ]);

    assert_eq!(batch, expected);
}
