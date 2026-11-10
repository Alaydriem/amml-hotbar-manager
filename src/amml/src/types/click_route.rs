use hotbar_manager_core::structs::ButtonAction;

// What the button event detour does with one pointer event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickRoute {
    // the game gets the event
    Pass,
    // the game does not get the event; nothing fires
    Swallow,
    // the game does not get the event, and the button's action fires (a release inside the pressed button)
    Fire(ButtonAction),
}
