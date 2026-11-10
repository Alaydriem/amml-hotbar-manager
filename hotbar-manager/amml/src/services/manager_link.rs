use std::path::Path;

use amm_client::link::{ApiTable, EventDrain, ManagerLink};
use amm_core::errors::LinkError;
use amm_core::structs::event::ModEvent;

use crate::services::HotbarDescriptor;

// the registration with the mod manager; `events` holds at most MAX_EVENTS_PER_TICK and is cleared at the start of
// every drain
pub struct HotbarLink {
    link: ManagerLink,
    events: Vec<ModEvent>,
}

impl HotbarLink {
    pub const MAX_DRAINS_PER_TICK: usize = 4;
    pub const MAX_EVENTS_PER_TICK: usize = Self::MAX_DRAINS_PER_TICK * EventDrain::CAPACITY;

    pub fn new(table: ApiTable, config_dir: &Path) -> Result<Self, LinkError> {
        let link = ManagerLink::without_frame(table, HotbarDescriptor::TEXT, config_dir)?;
        Ok(Self::from_link(link))
    }

    pub fn from_link(link: ManagerLink) -> Self {
        Self { link, events: Vec::with_capacity(Self::MAX_EVENTS_PER_TICK) }
    }

    // reads the queued manager events, at most MAX_EVENTS_PER_TICK; the rest wait for the next tick
    pub fn drain(&mut self) -> Vec<ModEvent> {
        self.events.clear();
        for _ in 0..Self::MAX_DRAINS_PER_TICK {
            let before = self.events.len();
            self.events.extend(self.link.events());
            if self.events.len() - before < EventDrain::CAPACITY {
                break;
            }
        }
        self.events.clone()
    }
}
