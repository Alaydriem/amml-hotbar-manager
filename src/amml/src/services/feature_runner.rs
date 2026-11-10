use hotbar_manager_core::services::{ItemFilter, PLAYER_MAIN, SlotModel, SortStepper, SortTick, StepPlanner};
use hotbar_manager_core::structs::{ButtonAction, ScreenKind, ScreenStep, Side, SlotAddr, SlotItem};

use crate::game::ScreenActions;
use crate::traits::ScreenSource;

// why an action did nothing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ignored {
    // an action is already queued, a sort runs, or a rotation runs
    Busy,
    // no supported screen is open
    NoScreen,
    // a screen the action does not work on (a sort on the player inventory, a dump without a container, a drop on a
    // container)
    WrongScreen,
    // the slots or the cursor of the screen could not be read
    ReadFailed,
    // a filtered action, and the cursor holds no stack to match
    NothingHeld,
    // the plan has no step
    NothingToDo,
    // a sort, and the cursor holds a stack
    CursorHeld,
    // the container handlers are off (an entry is missing) or cannot be called on this screen
    ActionsOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionResult {
    // a dump or a drop: every step ran in this frame; `calls` is the number of steps the game took
    Done { calls: usize },
    // a sort: it runs one step per frame from now on (`step_sort`)
    Started { sort: bool },
    Ignored(Ignored),
}

// how a sort ended, kept for the caller to log
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortEnd {
    pub steps: usize,
    // None when the sort ended Done
    pub reason: Option<&'static str>,
    // the cursor still holds a stack: one the sort did not take, or its own with no empty player slot to put it in
    // (closing the screen gives it back to the player)
    pub left_on_cursor: bool,
}

// Turns a button action or a keybind into steps of the game's own container handlers. A dump or a drop runs all its
// steps in its first frame. A sort runs one step per frame, each planned on a fresh read of the screen, until the
// container matches its target or the sort is stuck. The caller drops the sort when the screen closes or changes
// (`stop_sort`). Pure: the game is reached through `ScreenSource` and `ScreenActions`
#[derive(Default)]
pub struct FeatureRunner {
    sort: Option<SortStepper>,
    ended: Option<SortEnd>,
}

impl FeatureRunner {
    // the stop reason of a sort whose screen could not be read
    pub const READ_FAILED: &'static str = "the screen could not be read";
    // the stop reason of a sort whose screen closed or changed
    pub const SCREEN_GONE: &'static str = "the screen closed or changed";
    // the stop reason of a sort whose screen gives no container handlers
    pub const NO_HANDLERS: &'static str = "the container handlers cannot be called on this screen";

    pub fn new() -> Self {
        Self::default()
    }

    // whether `action` works on a screen of `kind`: the sort and the dumps on a container screen, the drops on the
    // player inventory
    pub fn fits(action: ButtonAction, kind: ScreenKind) -> bool {
        match action {
            ButtonAction::DropAll | ButtonAction::DropFiltered => kind == ScreenKind::PlayerInventory,
            ButtonAction::Sort
            | ButtonAction::DumpFromInventory
            | ButtonAction::DumpFromContainer
            | ButtonAction::DumpFromInventoryFiltered
            | ButtonAction::DumpFromContainerFiltered => kind.sortable(),
        }
    }

    // whether a sort runs
    pub fn sorting(&self) -> bool {
        self.sort.is_some()
    }

    // the sort is dropped from outside (the screen closed or changed, or it gives no handlers), and nothing more is
    // called. Gives how it ended, with `reason`; None when no sort ran. The cursor is not read here, so
    // `left_on_cursor` says only whether the sort's last step took a stack it had not put down yet
    pub fn stop_sort(&mut self, reason: &'static str) -> Option<SortEnd> {
        let stepper = self.sort.as_ref()?;
        let (steps, holding) = (stepper.steps(), stepper.holding());
        self.finish(steps, Some(reason), holding);
        self.ended.take()
    }

    // how the last sort ended, once
    pub fn take_sort_end(&mut self) -> Option<SortEnd> {
        self.ended.take()
    }

    // the first frame of an action on a screen of `kind`: reads the screen and runs a dump or a drop at once, or
    // starts a sort. A sort that runs, a wrong screen, an unreadable screen, an empty plan or (for a sort) a held
    // cursor ignore the action
    pub fn start(
        &mut self,
        action: ButtonAction,
        kind: ScreenKind,
        screen: &dyn ScreenSource,
        actions: &mut dyn ScreenActions,
    ) -> ActionResult {
        if self.sorting() {
            return ActionResult::Ignored(Ignored::Busy);
        }
        // a screen the action does not fit is not read
        if !Self::fits(action, kind) {
            return ActionResult::Ignored(Ignored::WrongScreen);
        }
        let Some(model) = screen.read() else { return ActionResult::Ignored(Ignored::ReadFailed) };
        if action == ButtonAction::Sort {
            return match screen.cursor() {
                None => ActionResult::Ignored(Ignored::ReadFailed),
                Some(Some(_)) => ActionResult::Ignored(Ignored::CursorHeld),
                Some(None) => {
                    self.sort = Some(SortStepper::new(model.container.len()));
                    self.ended = None;
                    ActionResult::Started { sort: true }
                }
            };
        }
        let filtered = matches!(
            action,
            ButtonAction::DumpFromInventoryFiltered
                | ButtonAction::DumpFromContainerFiltered
                | ButtonAction::DropFiltered
        );
        // the filter is the stack held on the cursor (the vanilla `#inventory_selected_item`); only the filtered
        // buttons read it
        let filter = if filtered {
            match screen.cursor() {
                None => return ActionResult::Ignored(Ignored::ReadFailed),
                Some(None) => return ActionResult::Ignored(Ignored::NothingHeld),
                Some(Some(held)) => Some(ItemFilter { id: held.id, aux: held.aux }),
            }
        } else {
            None
        };
        let steps = StepPlanner::transfer(action, &model, filter.as_ref());
        if steps.is_empty() {
            return ActionResult::Ignored(Ignored::NothingToDo);
        }
        let calls = steps.into_iter().filter(|&step| actions.run(step)).count();
        ActionResult::Done { calls }
    }

    // one frame of the running sort: reads the screen and runs the next step. None when no sort runs. A step the game
    // refuses (`run` gives false) changes nothing, so the stepper's unchanged-frame rule ends the sort. When the sort
    // ends Stuck with its own stack on the cursor, the stack goes into the first empty player slot (inventory, then
    // hotbar)
    pub fn step_sort(&mut self, screen: &dyn ScreenSource, actions: &mut dyn ScreenActions) -> Option<SortTick> {
        let stepper = self.sort.as_mut()?;
        let (Some(model), Some(cursor)) = (screen.read(), screen.cursor()) else {
            let (steps, holding) = (stepper.steps(), stepper.holding());
            self.finish(steps, Some(Self::READ_FAILED), holding);
            return Some(SortTick::Stuck);
        };
        let tick = stepper.next(&model.container, cursor.as_ref());
        let steps = stepper.steps();
        match tick {
            SortTick::Step(step) => {
                actions.run(step);
            }
            SortTick::Done => self.finish(steps, None, false),
            SortTick::Stuck => {
                let reason = stepper.stuck_reason().map(|reason| reason.text());
                let left = match cursor {
                    Some(_) if stepper.holds(cursor.as_ref()) => !Self::put_away(&model, actions),
                    Some(_) => true,
                    None => false,
                };
                self.finish(steps, reason, left);
            }
        }
        Some(tick)
    }

    fn finish(&mut self, steps: usize, reason: Option<&'static str>, left_on_cursor: bool) {
        self.sort = None;
        self.ended = Some(SortEnd { steps, reason, left_on_cursor });
    }

    // the cursor stack into the first empty player slot: the main rows (9..35), then the hotbar (0..8). False when no
    // slot is empty or the game refused the place
    fn put_away(model: &SlotModel, actions: &mut dyn ScreenActions) -> bool {
        let empty = PLAYER_MAIN.into_iter().chain(0..9).find(|&slot| Self::empty(model.player.get(usize::from(slot))));
        empty.is_some_and(|slot| actions.run(ScreenStep::PlaceAll(SlotAddr { side: Side::Player, slot })))
    }

    fn empty(slot: Option<&Option<SlotItem>>) -> bool {
        slot.is_some_and(Option::is_none)
    }
}
