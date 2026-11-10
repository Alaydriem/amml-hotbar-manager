use alaydriem_hotbar_manager::services::{ActionResult, FeatureRunner, Ignored, SortEnd};
use hotbar_manager_core::services::{ItemFilter, SortTick, StepPlanner, StuckReason};
use hotbar_manager_core::structs::{ButtonAction, ScreenKind, ScreenStep, Side, SlotAddr};

use crate::support::fake_actions::FakeActions;
use crate::support::fake_screen::{FakeScreen, model, stack};

fn chest(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Container, slot }
}

fn player(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Player, slot }
}

// a chest of 54 slots with a few stacks; the player holds stone in hotbar 0, cobblestone in hotbar 3 and
// main 10, stone in main 20
fn messy_chest() -> FakeScreen {
    let mut model = model(54);
    model.container[0] = stack("dirt", 0, 10, 100);
    model.container[1] = stack("cobblestone", 0, 20, 101);
    model.container[2] = stack("stone", 0, 30, 102);
    model.container[5] = stack("cobblestone", 0, 5, 103);
    model.player[0] = stack("stone", 0, 8, 110);
    model.player[3] = stack("cobblestone", 0, 9, 111);
    model.player[10] = stack("cobblestone", 0, 12, 112);
    model.player[20] = stack("stone", 0, 4, 113);
    FakeScreen::new(ScreenKind::LargeChest, model)
}

fn inventory_screen() -> FakeScreen {
    let mut model = model(0);
    model.player[0] = stack("stone", 0, 8, 110);
    model.player[3] = stack("cobblestone", 0, 9, 111);
    model.player[10] = stack("cobblestone", 0, 12, 112);
    model.player[20] = stack("stone", 0, 4, 113);
    model.player[21] = stack("stone", 1, 6, 114);
    FakeScreen::new(ScreenKind::PlayerInventory, model)
}

fn start(
    runner: &mut FeatureRunner,
    action: ButtonAction,
    screen: &FakeScreen,
    actions: &mut FakeActions,
) -> ActionResult {
    runner.start(action, screen.kind, screen, actions)
}

// runs the sort to its end, one step_sort per frame; gives the last tick and the number of frames
fn run_sort(runner: &mut FeatureRunner, screen: &FakeScreen, actions: &mut FakeActions) -> (SortTick, usize) {
    for frame in 1..=500 {
        let before = actions.steps.len();
        match runner.step_sort(screen, actions) {
            Some(SortTick::Step(step)) => {
                assert_eq!(actions.steps.len(), before + 1, "one step per frame");
                assert_eq!(actions.steps.last(), Some(&step));
            }
            Some(end) => return (end, frame),
            None => panic!("no sort runs"),
        }
    }
    panic!("the sort did not end");
}

#[test]
fn a_dump_runs_every_step_in_its_first_frame() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();
    let planned = StepPlanner::transfer(ButtonAction::DumpFromInventory, &screen.model.borrow().clone().unwrap(), None);
    assert!(!planned.is_empty());

    let result = start(&mut runner, ButtonAction::DumpFromInventory, &screen, &mut actions);

    assert_eq!(result, ActionResult::Done { calls: planned.len() });
    assert_eq!(actions.steps, planned);
    assert!(!runner.sorting());
    assert_eq!(runner.step_sort(&screen, &mut actions), None);
    assert_eq!(actions.steps.len(), planned.len(), "nothing runs on the next frame");
}

#[test]
fn every_dump_and_drop_runs_the_planned_steps() {
    // the filtered buttons match the stack held on the cursor: stone, aux 0
    let chest_screen = messy_chest();
    chest_screen.hold(stack("stone", 0, 1, 120));
    let inventory = inventory_screen();
    inventory.hold(stack("stone", 0, 1, 120));
    let stone = ItemFilter { id: "minecraft:stone".to_string(), aux: 0 };
    let cases = [
        (ButtonAction::DumpFromInventory, &chest_screen, None),
        (ButtonAction::DumpFromContainer, &chest_screen, None),
        (ButtonAction::DumpFromInventoryFiltered, &chest_screen, Some(&stone)),
        (ButtonAction::DumpFromContainerFiltered, &chest_screen, Some(&stone)),
        (ButtonAction::DropAll, &inventory, None),
        (ButtonAction::DropFiltered, &inventory, Some(&stone)),
    ];

    for (action, screen, filter) in cases {
        let mut runner = FeatureRunner::new();
        let mut actions = FakeActions::recording();
        let planned = StepPlanner::transfer(action, &screen.model.borrow().clone().unwrap(), filter);

        let result = start(&mut runner, action, screen, &mut actions);

        assert_eq!(result, ActionResult::Done { calls: planned.len() }, "{action:?}");
        assert_eq!(actions.steps, planned, "{action:?}");
    }
}

#[test]
fn dump_from_container_auto_places_every_filled_container_slot() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    start(&mut runner, ButtonAction::DumpFromContainer, &screen, &mut actions);

    // the game picks the targets (hotbar first, then the inventory, as a vanilla shift-click does)
    let expected: Vec<_> = [0, 1, 2, 5].map(|slot| ScreenStep::AutoPlace(chest(slot))).to_vec();
    assert_eq!(actions.steps, expected);
}

#[test]
fn a_filtered_action_with_nothing_held_runs_nothing() {
    // the selected hotbar slot (stone in hotbar 0) is not the filter any more
    let chest_screen = messy_chest();
    let inventory = inventory_screen();
    let filtered = [
        (ButtonAction::DumpFromInventoryFiltered, &chest_screen),
        (ButtonAction::DumpFromContainerFiltered, &chest_screen),
        (ButtonAction::DropFiltered, &inventory),
    ];

    for (action, screen) in filtered {
        let mut runner = FeatureRunner::new();
        let mut actions = FakeActions::recording();

        let result = start(&mut runner, action, screen, &mut actions);

        assert_eq!(result, ActionResult::Ignored(Ignored::NothingHeld), "{action:?}");
        assert!(actions.steps.is_empty(), "{action:?}");
    }
}

#[test]
fn a_filtered_action_with_an_unreadable_cursor_runs_nothing() {
    let screen = messy_chest();
    *screen.cursor.borrow_mut() = None;
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    let result = start(&mut runner, ButtonAction::DumpFromContainerFiltered, &screen, &mut actions);

    assert_eq!(result, ActionResult::Ignored(Ignored::ReadFailed));
    assert!(actions.steps.is_empty());
}

#[test]
fn the_unfiltered_buttons_do_not_read_the_cursor() {
    let chest_screen = messy_chest();
    *chest_screen.cursor.borrow_mut() = None;
    let inventory = inventory_screen();
    *inventory.cursor.borrow_mut() = None;

    for (action, screen) in [
        (ButtonAction::DumpFromInventory, &chest_screen),
        (ButtonAction::DumpFromContainer, &chest_screen),
        (ButtonAction::DropAll, &inventory),
    ] {
        let mut runner = FeatureRunner::new();
        let mut actions = FakeActions::recording();

        assert!(matches!(start(&mut runner, action, screen, &mut actions), ActionResult::Done { .. }), "{action:?}");
    }
}

#[test]
fn a_filtered_dump_matches_the_held_stack_and_leaves_it_on_the_cursor() {
    // hotbar 0 holds stone (the old selected-slot filter); the cursor holds cobblestone: the cobblestone stacks move
    let screen = messy_chest();
    screen.hold(stack("cobblestone", 0, 2, 120));
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    start(&mut runner, ButtonAction::DumpFromInventoryFiltered, &screen, &mut actions);
    assert_eq!(actions.steps, [ScreenStep::AutoPlace(player(10)), ScreenStep::AutoPlace(player(3))]);

    let mut actions = FakeActions::recording();
    start(&mut runner, ButtonAction::DumpFromContainerFiltered, &screen, &mut actions);
    assert_eq!(actions.steps, [ScreenStep::AutoPlace(chest(1)), ScreenStep::AutoPlace(chest(5))]);
    assert!(screen.held().is_some(), "the dumps do not touch the cursor stack");
}

#[test]
fn drop_filtered_matches_the_aux_of_the_held_stack() {
    let screen = inventory_screen();
    screen.hold(stack("stone", 1, 1, 120));
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    start(&mut runner, ButtonAction::DropFiltered, &screen, &mut actions);

    assert_eq!(actions.steps, [ScreenStep::DropCursor, ScreenStep::DropSlot(player(21))]);
}

#[test]
fn move_all_dumps_the_main_rows_then_the_hotbar() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    start(&mut runner, ButtonAction::DumpFromInventory, &screen, &mut actions);

    let expected: Vec<_> = [10, 20, 0, 3].map(|slot| ScreenStep::AutoPlace(player(slot))).to_vec();
    assert_eq!(actions.steps, expected);
}

#[test]
fn drop_filtered_with_a_held_cursor_drops_it_first() {
    let screen = inventory_screen();
    screen.hold(stack("cobblestone", 0, 3, 120));
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);

    let result = start(&mut runner, ButtonAction::DropFiltered, &screen, &mut actions);

    // the cursor first, then the cobblestone stacks it matches: main 10, then hotbar 3
    assert_eq!(
        actions.steps,
        [ScreenStep::DropCursor, ScreenStep::DropSlot(player(10)), ScreenStep::DropSlot(player(3))]
    );
    assert_eq!(result, ActionResult::Done { calls: 3 });
    assert_eq!(screen.held(), None);
}

#[test]
fn drop_all_drops_the_main_rows_and_never_the_hotbar() {
    let screen = inventory_screen();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    start(&mut runner, ButtonAction::DropAll, &screen, &mut actions);

    let expected: Vec<_> = [10, 20, 21].map(|slot| ScreenStep::DropSlot(player(slot))).to_vec();
    assert_eq!(actions.steps, expected);
}

#[test]
fn a_dump_does_not_touch_a_held_cursor() {
    let screen = messy_chest();
    screen.hold(stack("sand", 0, 3, 120));
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    start(&mut runner, ButtonAction::DumpFromInventory, &screen, &mut actions);

    assert!(!actions.steps.contains(&ScreenStep::DropCursor));
    assert!(actions.steps.iter().all(|step| matches!(step, ScreenStep::AutoPlace(_))));
}

#[test]
fn the_drops_work_only_on_the_player_inventory_and_the_dumps_only_on_a_container() {
    let cases = [
        (ButtonAction::DropAll, messy_chest()),
        (ButtonAction::DropFiltered, messy_chest()),
        (ButtonAction::Sort, inventory_screen()),
        (ButtonAction::DumpFromInventory, inventory_screen()),
        (ButtonAction::DumpFromContainer, inventory_screen()),
        (ButtonAction::DumpFromInventoryFiltered, inventory_screen()),
        (ButtonAction::DumpFromContainerFiltered, inventory_screen()),
    ];

    for (action, screen) in cases {
        let mut runner = FeatureRunner::new();
        let mut actions = FakeActions::recording();

        let result = start(&mut runner, action, &screen, &mut actions);

        assert_eq!(result, ActionResult::Ignored(Ignored::WrongScreen), "{action:?}");
        assert_eq!(screen.reads.get(), 0, "{action:?}: a screen the action does not fit is not read");
        assert!(actions.steps.is_empty(), "{action:?}");
    }
}

#[test]
fn an_unreadable_screen_runs_nothing() {
    let screen = messy_chest();
    *screen.model.borrow_mut() = None;
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    for action in [ButtonAction::Sort, ButtonAction::DumpFromContainer] {
        assert_eq!(start(&mut runner, action, &screen, &mut actions), ActionResult::Ignored(Ignored::ReadFailed));
    }
    assert!(actions.steps.is_empty());
}

#[test]
fn a_dump_with_nothing_to_move_runs_nothing() {
    let screen = FakeScreen::new(ScreenKind::SmallChest, model(27));
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    let result = start(&mut runner, ButtonAction::DumpFromContainer, &screen, &mut actions);

    assert_eq!(result, ActionResult::Ignored(Ignored::NothingToDo));
}

#[test]
fn sort_runs_one_step_per_frame_until_done() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);

    let result = start(&mut runner, ButtonAction::Sort, &screen, &mut actions);

    assert_eq!(result, ActionResult::Started { sort: true });
    assert!(runner.sorting());
    assert!(actions.steps.is_empty(), "the start itself calls nothing");
    let (end, frames) = run_sort(&mut runner, &screen, &mut actions);
    assert_eq!(end, SortTick::Done);
    assert_eq!(frames, actions.steps.len() + 1);
    // Done is given once; the sort is gone after it
    assert!(!runner.sorting());
    assert_eq!(runner.step_sort(&screen, &mut actions), None);
    assert_eq!(
        runner.take_sort_end(),
        Some(SortEnd { steps: actions.steps.len(), reason: None, left_on_cursor: false })
    );
    assert_eq!(runner.take_sort_end(), None);
    let ids: Vec<_> = screen.model.borrow().as_ref().unwrap().container[..4]
        .iter()
        .map(|slot| slot.as_ref().map(|item| (item.id.replace("minecraft:", ""), item.count)))
        .collect();
    assert_eq!(
        ids,
        [Some(("cobblestone".to_string(), 25)), Some(("dirt".to_string(), 10)), Some(("stone".to_string(), 30)), None]
    );
    assert_eq!(screen.held(), None);
}

#[test]
fn sort_with_a_held_cursor_is_ignored() {
    let screen = messy_chest();
    screen.hold(stack("sand", 0, 3, 120));
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    let result = start(&mut runner, ButtonAction::Sort, &screen, &mut actions);

    assert_eq!(result, ActionResult::Ignored(Ignored::CursorHeld));
    assert!(!runner.sorting());
    assert!(actions.steps.is_empty());
}

#[test]
fn sort_with_an_unreadable_cursor_is_ignored() {
    let screen = messy_chest();
    *screen.cursor.borrow_mut() = None;
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    assert_eq!(
        start(&mut runner, ButtonAction::Sort, &screen, &mut actions),
        ActionResult::Ignored(Ignored::ReadFailed)
    );
    assert!(!runner.sorting());
}

#[test]
fn an_action_while_a_sort_runs_is_busy() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);

    let second = start(&mut runner, ButtonAction::DumpFromContainer, &screen, &mut actions);

    assert_eq!(second, ActionResult::Ignored(Ignored::Busy));
    assert!(actions.steps.is_empty());
}

#[test]
fn a_cancelled_sort_runs_no_more_steps() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);
    runner.step_sort(&screen, &mut actions);
    let steps = actions.steps.len();

    let ended = runner.stop_sort(FeatureRunner::SCREEN_GONE);

    assert_eq!(ended.map(|end| (end.steps, end.reason)), Some((1, Some(FeatureRunner::SCREEN_GONE))));
    assert_eq!(runner.stop_sort(FeatureRunner::SCREEN_GONE), None, "no sort runs any more");
    assert!(!runner.sorting());
    assert_eq!(runner.step_sort(&screen, &mut actions), None);
    assert_eq!(actions.steps.len(), steps);
}

#[test]
fn a_refused_step_changes_nothing_and_the_sort_ends_stuck() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);
    actions.refuse = true;
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);

    let (end, frames) = run_sort(&mut runner, &screen, &mut actions);

    assert_eq!(end, SortTick::Stuck);
    assert_eq!(frames, 4, "3 unchanged frames, then Stuck");
    let ended = runner.take_sort_end().unwrap();
    assert_eq!(ended.reason, Some(StuckReason::Unchanged.text()));
    assert_eq!(ended.steps, 3);
}

#[test]
fn a_sort_that_cannot_read_the_screen_stops() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);
    *screen.model.borrow_mut() = None;

    assert_eq!(runner.step_sort(&screen, &mut actions), Some(SortTick::Stuck));
    assert!(!runner.sorting());
    assert_eq!(runner.take_sort_end().unwrap().reason, Some(FeatureRunner::READ_FAILED));
}

// a two-slot container: the sort takes the apple from slot 1 to put it in slot 0; then the game fills both slots
// with dirt, so the apple on the cursor has no room. The player slots in `filled` hold dirt too
fn stuck_holding_the_apple(filled: &[u8]) -> (FakeScreen, FeatureRunner, Vec<ScreenStep>) {
    let mut start_model = model(2);
    start_model.container[1] = stack("apple", 0, 5, 200);
    for &slot in filled {
        start_model.player[usize::from(slot)] = stack("dirt", 0, 64, 300);
    }
    let screen = FakeScreen::new(ScreenKind::SmallChest, start_model);
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);
    assert_eq!(runner.step_sort(&screen, &mut actions), Some(SortTick::Step(ScreenStep::TakeAll(chest(1)))));
    assert!(screen.held().is_some());
    {
        let mut shown = screen.model.borrow_mut();
        let shown = shown.as_mut().unwrap();
        shown.container[0] = stack("dirt", 0, 64, 301);
        shown.container[1] = stack("dirt", 0, 64, 302);
    }

    assert_eq!(runner.step_sort(&screen, &mut actions), Some(SortTick::Stuck));
    let steps = actions.steps;
    (screen, runner, steps)
}

#[test]
fn a_stuck_sort_puts_its_own_stack_into_the_first_empty_inventory_slot() {
    let (screen, mut runner, steps) = stuck_holding_the_apple(&[9, 10]);

    assert_eq!(steps.last(), Some(&ScreenStep::PlaceAll(player(11))));
    assert_eq!(screen.held(), None);
    assert_eq!(screen.slot(player(11)).map(|item| item.id), Some("minecraft:apple".to_string()));
    let ended = runner.take_sort_end().unwrap();
    assert_eq!(ended.reason, Some(StuckReason::NoRoom.text()));
    assert!(!ended.left_on_cursor);
}

#[test]
fn with_the_inventory_full_the_stuck_stack_goes_to_the_hotbar() {
    let mut filled: Vec<u8> = (9..36).collect();
    filled.push(0);
    let (screen, _, steps) = stuck_holding_the_apple(&filled);

    assert_eq!(steps.last(), Some(&ScreenStep::PlaceAll(player(1))));
    assert_eq!(screen.held(), None);
}

#[test]
fn with_no_empty_player_slot_the_stuck_stack_stays_on_the_cursor() {
    let filled: Vec<u8> = (0..36).collect();
    let (screen, mut runner, steps) = stuck_holding_the_apple(&filled);

    assert_eq!(steps.last(), Some(&ScreenStep::TakeAll(chest(1))), "no place step");
    assert!(screen.held().is_some());
    assert!(runner.take_sort_end().unwrap().left_on_cursor);
}

#[test]
fn a_stack_the_sort_did_not_take_is_never_put_down() {
    let mut start_model = model(2);
    start_model.container[1] = stack("apple", 0, 5, 200);
    let screen = FakeScreen::new(ScreenKind::SmallChest, start_model);
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);
    runner.step_sort(&screen, &mut actions);
    // the game shows another stack on the cursor
    screen.hold(stack("sand", 0, 7, 400));

    assert_eq!(runner.step_sort(&screen, &mut actions), Some(SortTick::Stuck));

    assert_eq!(actions.steps, [ScreenStep::TakeAll(chest(1))]);
    let ended = runner.take_sort_end().unwrap();
    assert_eq!(ended.reason, Some("the cursor holds a stack the sort did not take"));
    assert!(ended.left_on_cursor);
}

#[test]
fn a_read_failure_with_an_empty_cursor_does_not_claim_a_held_stack() {
    let screen = messy_chest();
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);
    *screen.model.borrow_mut() = None;

    runner.step_sort(&screen, &mut actions);

    assert!(!runner.take_sort_end().unwrap().left_on_cursor);
}

#[test]
fn a_read_failure_right_after_a_take_says_a_stack_may_be_held() {
    let mut start_model = model(2);
    start_model.container[1] = stack("apple", 0, 5, 200);
    let screen = FakeScreen::new(ScreenKind::SmallChest, start_model);
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::applying(&screen);
    start(&mut runner, ButtonAction::Sort, &screen, &mut actions);
    assert_eq!(runner.step_sort(&screen, &mut actions), Some(SortTick::Step(ScreenStep::TakeAll(chest(1)))));
    *screen.model.borrow_mut() = None;

    runner.step_sort(&screen, &mut actions);

    assert!(runner.take_sort_end().unwrap().left_on_cursor);
}

#[test]
fn stop_sort_with_no_sort_gives_nothing() {
    let mut runner = FeatureRunner::new();

    assert_eq!(runner.stop_sort(FeatureRunner::SCREEN_GONE), None);
}

#[test]
fn drop_filtered_with_no_matching_slot_drops_only_the_held_stack() {
    let mut shown = model(0);
    shown.player[0] = stack("stone", 0, 8, 110);
    shown.player[12] = stack("stone", 0, 64, 111);
    shown.player[30] = stack("stone", 0, 3, 112);
    let screen = FakeScreen::new(ScreenKind::PlayerInventory, shown);
    screen.hold(stack("apple", 0, 4, 120));
    let mut runner = FeatureRunner::new();
    let mut actions = FakeActions::recording();

    let result = start(&mut runner, ButtonAction::DropFiltered, &screen, &mut actions);

    assert_eq!(actions.steps, [ScreenStep::DropCursor]);
    assert_eq!(result, ActionResult::Done { calls: 1 });
}
