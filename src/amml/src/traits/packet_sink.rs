// where the move engine sends its item stack requests: the game's packet sender, or a test's fake. `bytes` is the
// request payload without the packet header; false when the request was not sent
pub trait PacketSink {
    fn send(&mut self, bytes: &[u8]) -> bool;
}
