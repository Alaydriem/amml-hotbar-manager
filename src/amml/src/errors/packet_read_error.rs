use std::fmt;

// a request packet was not made or did not read the bytes the mod wrote, so it is not sent
#[derive(Debug, PartialEq, Eq)]
pub enum PacketReadError {
    NotMade,
    NoReadSlot,
    Refused { read_to: u64, length: usize },
}

impl fmt::Display for PacketReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotMade => write!(f, "the game made no packet of that type"),
            Self::NoReadSlot => write!(f, "the packet has no read function"),
            Self::Refused { read_to, length } => write!(f, "the packet refused the bytes: read {read_to} of {length}"),
        }
    }
}

impl std::error::Error for PacketReadError {}
