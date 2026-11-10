use alaydriem_hotbar_manager::game::ScreenActions;
use hotbar_manager_core::structs::ScreenStep;

use crate::support::fake_screen::FakeScreen;

// records every step it is given; when made with a screen it also applies the step to it, as the game would. A
// refusing fake returns false and changes nothing
pub struct FakeActions<'a> {
    pub steps: Vec<ScreenStep>,
    screen: Option<&'a FakeScreen>,
    pub refuse: bool,
}

impl<'a> FakeActions<'a> {
    pub fn recording() -> Self {
        Self { steps: Vec::new(), screen: None, refuse: false }
    }

    pub fn applying(screen: &'a FakeScreen) -> Self {
        Self { steps: Vec::new(), screen: Some(screen), refuse: false }
    }
}

impl ScreenActions for FakeActions<'_> {
    fn run(&mut self, step: ScreenStep) -> bool {
        self.steps.push(step);
        if self.refuse {
            return false;
        }
        if let Some(screen) = self.screen {
            screen.apply(step);
        }
        true
    }
}
