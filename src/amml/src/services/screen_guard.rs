use amml_api::{Screen, ScreenId};

// what to do with the action that is queued or running
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Keep,
    // the screen the action started on is gone: drop the sort and the queued action at once
    Cancel,
}

// Tracks the identity of the open screen: the top screen of the loader (read on every client tick and given by
// `ScreenChanged`) and the id of the screen that the frames of the action and the changes name. An action is bound to the identity it
// started on. Any change of the top screen (a close, a switch), a change to another screen id, or a frame of another
// screen id (a switch to another container that never shows the HUD between) cancels it. The time between frames does
// not matter: a frame hitch or a low frame rate changes nothing. A toast on top changes nothing. Pure
#[derive(Debug, Default)]
pub struct ScreenGuard {
    // the top screen of the last read that was not a toast; None before the first read
    top: Option<Screen>,
    bound: Option<Bound>,
}

#[derive(Debug)]
struct Bound {
    top: Option<Screen>,
    screen: Option<ScreenId>,
}

impl ScreenGuard {
    pub fn new() -> Self {
        Self::default()
    }

    // the top screen of this tick. Cancel when an action is bound and the screen differs from the one it started on
    pub fn top_changed(&mut self, screen: Screen) -> Verdict {
        if screen == Screen::Toast {
            return Verdict::Keep;
        }
        self.top = Some(screen);
        let moved = self.bound.as_ref().is_some_and(|bound| bound.top != Some(screen));
        if moved {
            self.bound = None;
            return Verdict::Cancel;
        }
        Verdict::Keep
    }

    // a `ScreenChanged` to the screen `new` with the top screen `screen`. Cancel when an action is bound and either
    // differs from what it started on, or when no screen is on top any more
    pub fn screen_changed(&mut self, new: ScreenId, screen: Screen) -> Verdict {
        if screen == Screen::Toast {
            return Verdict::Keep;
        }
        let by_top = self.top_changed(screen);
        let gone = self.bound.as_ref().is_some_and(|bound| match bound.screen {
            Some(known) => known != new,
            None => new == ScreenId::NONE,
        });
        if gone {
            self.bound = None;
            return Verdict::Cancel;
        }
        by_top
    }

    // whether the top screen is known
    pub fn top_known(&self) -> bool {
        self.top.is_some()
    }

    // whether a screen that shows a container is on top
    pub fn container_open(&self) -> bool {
        self.top == Some(Screen::Container)
    }

    // whether the HUD is the top screen: no menu is open and the world is shown. False while the top screen is not
    // known
    pub fn hud(&self) -> bool {
        self.top == Some(Screen::Hud)
    }

    // an action is queued or started: it is bound to the current top screen, and to the id of the screen of its first
    // frame
    pub fn bind(&mut self) {
        self.bound = Some(Bound { top: self.top, screen: None });
    }

    // the action ended or was dropped
    pub fn release(&mut self) {
        self.bound = None;
    }

    // a frame of the screen `screen`. The first frame of a bound action gives it its screen id; a frame of any other
    // screen cancels it
    pub fn frame(&mut self, screen: ScreenId) -> Verdict {
        let Some(bound) = self.bound.as_mut() else { return Verdict::Keep };
        match bound.screen {
            None => {
                bound.screen = Some(screen);
                Verdict::Keep
            }
            Some(known) if known == screen => Verdict::Keep,
            Some(_) => {
                self.bound = None;
                Verdict::Cancel
            }
        }
    }
}
