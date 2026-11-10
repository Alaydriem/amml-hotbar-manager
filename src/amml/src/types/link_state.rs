// where the link to the mod manager stands
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    // enabled; the next tick reaches the manager
    Waiting,
    Attached,
    // the manager is missing, too old or refused the mod; the mod stays off until the next enable
    Off,
    // the mod is disabled; no tick reaches the manager until the next enable
    Disabled,
}

impl LinkState {
    pub fn to_u8(self) -> u8 {
        match self {
            Self::Waiting => 0,
            Self::Attached => 1,
            Self::Off => 2,
            Self::Disabled => 3,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Attached,
            2 => Self::Off,
            3 => Self::Disabled,
            _ => Self::Waiting,
        }
    }
}
