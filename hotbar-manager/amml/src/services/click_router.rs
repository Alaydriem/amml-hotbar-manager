use hotbar_manager_core::services::ButtonLayout;
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, PanelRect, ScreenKind};

use crate::types::{ButtonLook, ClickRoute, InputMode, UiProfile};

// Decides which pointer events the mod buttons take (docs/findings/spike-click/report.md section 6). Pure: the
// button event detour hands it the view, the pointer index (the place of the event's id in
// `ScreenView::mPointerButtonIds`), the state, the button under the pointer and the input mode.
//
// - One held flag per pointer index: the button its press began on, so its release is swallowed too.
// - A new press of a pointer clears that pointer's stale flag first (its release was lost, for example after
//   alt-tab); without this, a later vanilla release would be swallowed.
// - Every flag clears on a view change.
// - The action fires on the release of the primary pointer (index 0) inside the button its press began on. A
//   swallowed press always swallows its release, even off the button and in any input mode.
// - A press is taken only in a pointer input mode; in gamepad mode the game sends it to the focused control.
//   With no `ScreenView.mInputMode` entry the mode is None, and a press over a button is taken.
pub struct ClickRouter {
    view: usize,
    held: [Option<ButtonAction>; Self::POINTERS],
}

impl Default for ClickRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClickRouter {
    // the length of `ScreenView::mPointerButtonIds` (`std::array<uint, 2>`)
    pub const POINTERS: usize = 2;
    // the pointer whose release fires an action (the left mouse button)
    pub const PRIMARY: usize = 0;

    pub fn new() -> Self {
        Self { view: 0, held: [None; Self::POINTERS] }
    }

    // the buttons of a screen: the layout of its kind in the Classic profile, none in Pocket
    pub fn buttons(kind: ScreenKind, profile: UiProfile) -> Vec<ButtonSpec> {
        match profile {
            UiProfile::Classic => ButtonLayout::for_screen(kind),
            UiProfile::Pocket => Vec::new(),
        }
    }

    // the button under the pointer (GUI units, screen space); each rect is half-open, [x, x + w) by [y, y + h), so a
    // shared edge belongs to one button only and Sort ends where the close button starts (+153)
    pub fn hit(specs: &[ButtonSpec], panel: PanelRect, pointer_units: (f32, f32)) -> Option<ButtonAction> {
        let (px, py) = pointer_units;
        specs
            .iter()
            .find(|spec| {
                let rect = panel.resolve(spec);
                px >= rect.x && px < rect.x + rect.w && py >= rect.y && py < rect.y + rect.h
            })
            .map(|spec| spec.action)
    }

    // the view the events come from; a different view clears every held flag
    pub fn observe_view(&mut self, view: usize) {
        if self.view != view {
            self.clear();
            self.view = view;
        }
    }

    // the view shows no buttons any more (its screen closed, or it turned into another screen); if it is the view
    // the flags belong to, they clear
    pub fn forget_view(&mut self, view: usize) {
        if self.view == view {
            self.clear();
        }
    }

    // what happens to one pointer event of `view`
    pub fn route(
        &mut self,
        view: usize,
        pointer: usize,
        pressed: bool,
        over: Option<ButtonAction>,
        mode: Option<InputMode>,
    ) -> ClickRoute {
        self.observe_view(view);
        let Some(held) = self.held.get_mut(pointer) else { return ClickRoute::Pass };
        if pressed {
            *held = None;
            if !mode.is_none_or(InputMode::uses_pointer) {
                return ClickRoute::Pass;
            }
            return match over {
                Some(action) => {
                    *held = Some(action);
                    ClickRoute::Swallow
                }
                None => ClickRoute::Pass,
            };
        }
        match held.take() {
            Some(action) if over == Some(action) && pointer == Self::PRIMARY => ClickRoute::Fire(action),
            Some(_) => ClickRoute::Swallow,
            None => ClickRoute::Pass,
        }
    }

    // whether a pointer holds a press that began on `view`
    pub fn holds(&self, view: usize) -> bool {
        view == self.view && self.held.iter().any(Option::is_some)
    }

    // the button drawn pressed on `view`: the one under the pointer, if a pointer holds it
    pub fn pressed(&self, view: usize, over: Option<ButtonAction>) -> Option<ButtonAction> {
        over.filter(|action| view == self.view && self.held.contains(&Some(*action)))
    }

    // the look of `action` on `view`: Pressed only when held and the pointer is inside, else Hover when inside, else
    // Default
    pub fn look(&self, view: usize, action: ButtonAction, over: Option<ButtonAction>) -> ButtonLook {
        if over != Some(action) {
            ButtonLook::Default
        } else if self.pressed(view, over).is_some() {
            ButtonLook::Pressed
        } else {
            ButtonLook::Hover
        }
    }

    pub fn clear(&mut self) {
        self.held = [None; Self::POINTERS];
    }
}
