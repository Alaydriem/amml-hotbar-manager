use amm_client::link::{LinkState, ManagerLink};
use amm_core::errors::LinkError;
use amm_core::structs::event::ModEvent;
use amml_api::Ctx;

use crate::services::HotbarDescriptor;

// the registration with the mod manager and the events read this tick; `events` is cleared at the start of every drain,
// and the link itself keeps a bounded number of unread events. Dropping the link leaves the manager
pub struct HotbarLink {
    link: ManagerLink,
    events: Vec<ModEvent>,
}

impl HotbarLink {
    // the link waits for the manager's tables; `state` says when it is connected
    pub fn start(ctx: &Ctx) -> Result<Self, LinkError> {
        let link = ManagerLink::start_without_frame(ctx, HotbarDescriptor::TEXT)?;
        Ok(Self { link, events: Vec::new() })
    }

    pub fn state(&self) -> LinkState {
        self.link.state()
    }

    // reads every queued manager event
    pub fn drain(&mut self) -> Vec<ModEvent> {
        self.events.clear();
        self.events.extend(self.link.events());
        self.events.clone()
    }
}
