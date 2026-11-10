use amml_game::ItemStackResponsePacketLayout;

// Reads the server's answers out of a live ItemStackResponsePacket through amml-game's layout mirror, as stratum
// does. The handler gets the decoded packet, not the wire bytes, so the inferred wire layout of the 9a report is not
// parsed here. A packet that does not look sane gives no answers: the request in flight then times out, which counts
// as refused
pub struct StackResponses;

impl StackResponses {
    // more answers than any packet of one client carries; a larger count is a misread
    const MAX_ANSWERS: usize = 256;
    // ItemStackNetResult::Success
    const SUCCESS: u8 = 0;

    // (request id, accepted) for each answer of `packet`
    pub fn answers(packet: &ItemStackResponsePacketLayout) -> Vec<(i32, bool)> {
        if packet.responses.len() > Self::MAX_ANSWERS {
            curia::warn!("item stack response not read", { answers: packet.responses.len() });
            return Vec::new();
        }
        packet
            .responses
            .as_slice()
            .iter()
            .map(|response| (response.client_request_id, response.result == Self::SUCCESS))
            .collect()
    }
}
