use hotbar_manager_core::structs::ScreenStep;

// Runs one step of an action on the open screen. False when the step could not be called or the game refused it
// (a swap gives the game's own result). The game gives it through `ContainerActions`; the tests give a fake
pub trait ScreenActions {
    fn run(&mut self, step: ScreenStep) -> bool;
}
