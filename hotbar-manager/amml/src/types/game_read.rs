use amml_game::{EntryId, EntryNeed, EntryResolver, GameEntries, GameError};
use amml_sdk::SdkError;

use crate::traits::GameHost;

// The entries of the SDK's screen wrappers that the button actions read the open screen with: the view's controller
// (`ScreenView` anchors its mirror on the pointer entry), the container test, the slot's visual stack, and the item
// name offset. They resolve once, at enable. A missing one turns off reading the screen, with one warning that names
// it (the buttons still draw; their actions do nothing)
pub struct GameRead;

impl GameRead {
    pub const FEATURE: &'static str = "reading the open screen";
    const IDS: [EntryId; 4] = [
        EntryId::SCREEN_VIEW_POINTER_LOCATION_PREVIOUS,
        EntryId::SCREEN_CONTROLLER_IS_CONTAINER_SCREEN,
        EntryId::CONTAINER_SCREEN_CONTROLLER_GET_VISUAL_ITEM_STACK_IMPL,
        EntryId::ITEM_FULL_NAME,
    ];
    pub const ENTRIES: [&'static str; 4] =
        [Self::IDS[0].as_str(), Self::IDS[1].as_str(), Self::IDS[2].as_str(), Self::IDS[3].as_str()];

    // the entries of the running build; a missing entry is not an error here, `missing` names it
    pub fn resolve<H: GameHost>(host: &H) -> Result<GameEntries, GameError> {
        let wanted: Vec<(EntryId, EntryNeed)> =
            Self::IDS.iter().map(|id| (*id, EntryNeed::Optional { feature: Self::FEATURE })).collect();
        GameEntries::resolve(&HostResolver(host), &wanted)
    }

    // the first entry that did not resolve
    pub fn missing(entries: &GameEntries) -> Option<&'static str> {
        Self::IDS.iter().find(|id| entries.get(**id).is_none()).map(EntryId::as_str)
    }
}

// the host as the resolver `GameEntries` takes
struct HostResolver<'a, H: GameHost>(&'a H);

impl<H: GameHost> EntryResolver for HostResolver<'_, H> {
    fn resolve(&self, id: &str) -> Result<u64, SdkError> {
        self.0.resolve(id)
    }
}
