use hotbar_manager_core::structs::ScreenKind;

use crate::services::ScreenDetector;

// what to do with the action that is queued or running
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Keep,
    // the screen the action started on is gone: drop the sort and the queued action at once
    Cancel,
}

// Tracks the identity of the open screen: the name `ClientInstance::getTopScreenName` gives, read on every client tick,
// and the controller address of the frames that draw it. An action is bound to the identity it started on. Any change
// of the name (a close, a switch), or a frame of another controller (a switch to another container of the same kind
// that never shows the HUD between), cancels it. The time between frames does not matter: a frame hitch or a low
// frame rate changes nothing. A toast on top changes nothing. Pure
#[derive(Debug, Default)]
pub struct ScreenGuard {
    // the top screen of the last tick; None before the first read
    top: Option<String>,
    bound: Option<Bound>,
}

#[derive(Debug)]
struct Bound {
    top: Option<String>,
    controller: Option<usize>,
}

impl ScreenGuard {
    // a notice (an achievement) drawn over the screen under it: no change
    pub const TOAST: &'static str = "toast_screen";
    // no menu is open
    pub const HUD: &'static str = "hud_screen";
    pub const START: &'static str = "start_screen";

    pub fn new() -> Self {
        Self::default()
    }

    // the top screen of this tick. Cancel when an action is bound and the name differs from the one it started on
    pub fn top_changed(&mut self, name: &str) -> Verdict {
        if name == Self::TOAST {
            return Verdict::Keep;
        }
        if self.top.as_deref() != Some(name) {
            self.top = Some(name.to_string());
        }
        let moved = self.bound.as_ref().is_some_and(|bound| bound.top.as_deref() != Some(name));
        if moved {
            self.bound = None;
            return Verdict::Cancel;
        }
        Verdict::Keep
    }

    // whether the top screen is known
    pub fn top_known(&self) -> bool {
        self.top.is_some()
    }

    // whether a menu screen is open, by the top screen name: not the HUD, not the title screen, not nothing
    pub fn menu_open(&self) -> bool {
        self.top.as_deref().is_some_and(|name| !name.is_empty() && name != Self::HUD && name != Self::START)
    }

    // whether the HUD is the top screen: no menu is open and the world is shown. False while the top screen is not
    // known
    pub fn hud(&self) -> bool {
        self.top.as_deref() == Some(Self::HUD)
    }

    // the kind of the top screen, Other when its name is not one the mod knows (or no name was read)
    pub fn top_kind(&self) -> ScreenKind {
        self.top.as_deref().map_or(ScreenKind::Other, ScreenDetector::kind_of)
    }

    // an action is queued or started: it is bound to the current screen, and to the controller of the first frame
    pub fn bind(&mut self) {
        self.bound = Some(Bound { top: self.top.clone(), controller: None });
    }

    // the action ended or was dropped
    pub fn release(&mut self) {
        self.bound = None;
    }

    // a frame of the open screen drawn by the controller at `controller`. The first frame of a bound action gives it
    // its controller; a frame of any other controller cancels it
    pub fn frame(&mut self, controller: usize) -> Verdict {
        let Some(bound) = self.bound.as_mut() else { return Verdict::Keep };
        match bound.controller {
            None => {
                bound.controller = Some(controller);
                Verdict::Keep
            }
            Some(known) if known == controller => Verdict::Keep,
            Some(_) => {
                self.bound = None;
                Verdict::Cancel
            }
        }
    }
}
