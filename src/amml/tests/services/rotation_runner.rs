use alaydriem_hotbar_manager::services::{MoveEngine, RotationEnd, RotationIgnored, RotationResult, RotationRunner};
use alaydriem_hotbar_manager::types::MoveEnd;
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{Direction, ItemMove, ScreenKind, Side, SlotAddr};

use crate::support::fake_link::{FakeLink, Sent};
use crate::support::fake_screen::{FakeScreen, model, stack};
use crate::support::fake_sender::FakePredictor;

const FIRST: i32 = MoveEngine::<FakePredictor>::FIRST_REQUEST_ID;
const CONTAINER: i8 = 5;
const INVENTORY: i8 = RotationRunner::INVENTORY_TYPE;

fn engine() -> MoveEngine<FakePredictor> {
    MoveEngine::new(FakePredictor::default())
}

// one stack in the hotbar: rotating down moves it to the first main row, one request
fn one_stack() -> SlotModel {
    let mut model = model(0);
    model.player[0] = stack("stone", 0, 4, 9);
    model
}

fn screen(model: SlotModel) -> FakeScreen {
    FakeScreen::new(ScreenKind::PlayerInventory, model)
}

// the runner after Interact was sent and the server answered with its open (hidden), nothing else done
fn opened(link: &mut FakeLink, engine: &MoveEngine<FakePredictor>) -> RotationRunner {
    let mut runner = RotationRunner::new();
    assert_eq!(runner.request(Direction::Down, 1.0, false, engine, link), RotationResult::Started);
    assert!(runner.on_container_open(1.1, CONTAINER, INVENTORY));
    runner
}

// the runner with its first request in flight
fn running(link: &mut FakeLink, engine: &mut MoveEngine<FakePredictor>, source: &FakeScreen) -> RotationRunner {
    let mut runner = opened(link, engine);
    assert_eq!(runner.tick(1.2, true, Some(source), engine, link), None);
    assert!(engine.busy());
    runner
}

#[test]
fn rotation_ignored_with_screen_open() {
    let (mut link, engine) = (FakeLink::default(), engine());
    let mut runner = RotationRunner::new();

    let result = runner.request(Direction::Down, 1.0, true, &engine, &mut link);

    assert_eq!(result, RotationResult::Ignored(RotationIgnored::ScreenOpen));
    assert!(link.sent.is_empty());
    assert!(!runner.active());
}

#[test]
fn rotation_ignored_while_engine_busy() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let from = SlotAddr { side: Side::Player, slot: 0 };
    let to = SlotAddr { side: Side::Player, slot: 9 };
    let moves = vec![ItemMove::Place { from, to, count: 4 }];
    assert!(engine.start(0.5, moves, &one_stack(), ScreenKind::PlayerInventory));
    let mut runner = RotationRunner::new();

    let result = runner.request(Direction::Down, 1.0, false, &engine, &mut link);

    assert_eq!(result, RotationResult::Ignored(RotationIgnored::Busy));
    assert!(link.sent.is_empty());
}

#[test]
fn rotation_ignored_while_a_rotation_runs() {
    let (mut link, engine) = (FakeLink::default(), engine());
    let mut runner = opened(&mut link, &engine);

    let result = runner.request(Direction::Up, 1.3, false, &engine, &mut link);

    assert_eq!(result, RotationResult::Ignored(RotationIgnored::Busy));
    assert_eq!(link.sent, [Sent::Open]);
}

#[test]
fn rotation_ignored_when_the_open_cannot_be_sent() {
    let (mut link, engine) = (FakeLink { refuse_open: true, ..FakeLink::default() }, engine());
    let mut runner = RotationRunner::new();

    let result = runner.request(Direction::Down, 1.0, false, &engine, &mut link);

    assert_eq!(result, RotationResult::Ignored(RotationIgnored::NotSent));
    assert!(!runner.active());
}

#[test]
fn rotation_opens_then_closes_hidden_inventory() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = running(&mut link, &mut engine, &source);

    engine.on_response(FIRST, true);
    let end = runner.tick(1.3, true, Some(&source), &mut engine, &mut link);

    assert_eq!(link.sent, [Sent::Open, Sent::Request, Sent::Close(CONTAINER)]);
    assert_eq!(end, Some(RotationEnd::Done));
    assert!(!runner.active());
}

#[test]
fn no_request_goes_out_before_the_servers_open_arrives() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = RotationRunner::new();
    runner.request(Direction::Down, 1.0, false, &engine, &mut link);

    let end = runner.tick(1.1, true, Some(&source), &mut engine, &mut link);

    assert_eq!(end, None);
    assert_eq!(link.sent, [Sent::Open]);
}

#[test]
fn an_accepted_run_ends_with_a_close() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = running(&mut link, &mut engine, &source);

    engine.on_response(FIRST, true);
    runner.tick(1.3, true, Some(&source), &mut engine, &mut link);

    assert_eq!(link.sent.last(), Some(&Sent::Close(CONTAINER)));
}

#[test]
fn a_rejected_run_ends_with_a_close() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = running(&mut link, &mut engine, &source);

    engine.on_response(FIRST, false);
    let end = runner.tick(1.3, true, Some(&source), &mut engine, &mut link);

    assert_eq!(link.sent, [Sent::Open, Sent::Request, Sent::Close(CONTAINER)]);
    let Some(RotationEnd::Moves(outcome)) = end else { panic!("{end:?}") };
    assert!(matches!(outcome.end, MoveEnd::Rejected { timed_out: false, .. }));
}

#[test]
fn a_run_that_times_out_ends_with_a_close() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = running(&mut link, &mut engine, &source);

    let late = 1.2 + MoveEngine::<FakePredictor>::BATCH_TIMEOUT_SECONDS;
    let end = runner.tick(late, true, Some(&source), &mut engine, &mut link);

    assert_eq!(link.sent, [Sent::Open, Sent::Request, Sent::Close(CONTAINER)]);
    let Some(RotationEnd::Moves(outcome)) = end else { panic!("{end:?}") };
    assert!(matches!(outcome.end, MoveEnd::Rejected { timed_out: true, .. }));
}

#[test]
fn a_screen_that_opens_mid_run_cancels_the_engine_and_closes() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = running(&mut link, &mut engine, &source);

    let end = runner.tick(1.3, false, Some(&source), &mut engine, &mut link);

    assert!(!engine.busy());
    assert_eq!(link.sent, [Sent::Open, Sent::Request, Sent::Close(CONTAINER)]);
    let Some(RotationEnd::Moves(outcome)) = end else { panic!("{end:?}") };
    assert_eq!(outcome.end, MoveEnd::Cancelled);
    assert!(!runner.active());
}

#[test]
fn a_screen_that_opens_before_the_servers_open_sends_no_request_and_closes() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = opened(&mut link, &engine);

    let end = runner.tick(1.2, false, Some(&source), &mut engine, &mut link);

    assert_eq!(link.sent, [Sent::Open, Sent::Close(CONTAINER)]);
    assert_eq!(end, Some(RotationEnd::Cancelled));
}

#[test]
fn rows_with_no_item_open_and_close_without_a_request() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(model(0));
    let mut runner = opened(&mut link, &engine);

    let end = runner.tick(1.2, true, Some(&source), &mut engine, &mut link);

    assert_eq!(link.sent, [Sent::Open, Sent::Close(CONTAINER)]);
    assert_eq!(end, Some(RotationEnd::Done));
}

#[test]
fn an_unreadable_inventory_closes_without_a_request() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let mut runner = opened(&mut link, &engine);

    let end = runner.tick(1.2, true, None, &mut engine, &mut link);

    assert_eq!(link.sent, [Sent::Open, Sent::Close(CONTAINER)]);
    assert_eq!(end, Some(RotationEnd::NoRead));
}

#[test]
fn a_server_that_never_opens_ends_the_wait_and_hides_nothing_late() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = RotationRunner::new();
    runner.request(Direction::Down, 1.0, false, &engine, &mut link);

    let late = 1.0 + RotationRunner::ANSWER_WINDOW_SECONDS + 0.1;
    let end = runner.tick(late, true, Some(&source), &mut engine, &mut link);

    assert_eq!(end, Some(RotationEnd::NoAnswer));
    assert!(!runner.active());
    assert!(!runner.on_container_open(late, CONTAINER, INVENTORY));
}

#[test]
fn only_the_inventory_open_that_was_asked_for_is_hidden() {
    let (mut link, engine) = (FakeLink::default(), engine());
    let mut runner = RotationRunner::new();
    assert!(!runner.on_container_open(1.0, CONTAINER, INVENTORY));
    runner.request(Direction::Down, 1.0, false, &engine, &mut link);

    assert!(!runner.on_container_open(1.1, CONTAINER, 0));
    assert!(runner.on_container_open(1.1, CONTAINER, INVENTORY));
    assert!(!runner.on_container_open(1.1, CONTAINER, INVENTORY));
}

#[test]
fn the_servers_answer_to_the_close_is_hidden_once() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(model(0));
    let mut runner = opened(&mut link, &engine);
    assert!(!runner.on_container_close(1.0, CONTAINER));
    runner.tick(1.2, true, Some(&source), &mut engine, &mut link);

    assert!(!runner.on_container_close(1.3, CONTAINER + 1));
    assert!(runner.on_container_close(1.3, CONTAINER));
    assert!(!runner.on_container_close(1.3, CONTAINER));
}

#[test]
fn reset_gives_the_container_to_close_while_the_inventory_is_open() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(one_stack());
    let mut runner = running(&mut link, &mut engine, &source);

    assert_eq!(runner.reset(), Some(CONTAINER));
    assert!(!runner.active());
    assert_eq!(runner.reset(), None);
}

#[test]
fn a_close_of_another_container_is_not_hidden() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(model(0));
    let mut runner = opened(&mut link, &engine);
    runner.tick(1.2, true, Some(&source), &mut engine, &mut link);

    assert!(!runner.on_container_close(1.3, CONTAINER + 1));
    assert!(!runner.on_container_close(1.3, 0));
}

#[test]
fn a_close_after_the_window_has_closed_is_not_hidden() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(model(0));
    let mut runner = opened(&mut link, &engine);
    runner.tick(1.2, true, Some(&source), &mut engine, &mut link);

    let late = 1.2 + RotationRunner::ANSWER_WINDOW_SECONDS + 0.1;
    assert!(!runner.on_container_close(late, CONTAINER));
}

#[test]
fn the_window_closes_at_the_first_matching_close() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let source = screen(model(0));
    let mut runner = opened(&mut link, &engine);
    runner.tick(1.2, true, Some(&source), &mut engine, &mut link);

    assert!(runner.on_container_close(1.3, CONTAINER));
    // the player's own close of the same container id, a moment later, goes to the game
    assert!(!runner.on_container_close(1.4, CONTAINER));
}

#[test]
fn a_rotation_waits_while_a_player_slot_reads_net_id_0() {
    // a move of an earlier screen action still waits for its answer: the moved stack reads net id 0
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let mut waiting = one_stack();
    waiting.player[14] = stack("dirt", 0, 3, 0);
    let source = screen(waiting);
    let mut runner = opened(&mut link, &engine);

    let end = runner.tick(1.2, true, Some(&source), &mut engine, &mut link);

    assert_eq!(end, Some(RotationEnd::MoveOnItsWay));
    assert!(!engine.busy(), "nothing is planned or sent");
    assert_eq!(link.sent, [Sent::Open, Sent::Close(CONTAINER)], "the hidden inventory is closed again");
    assert!(!runner.active());

    // the answers are in (the stack has its net id): the next press rotates
    let mut answered = one_stack();
    answered.player[14] = stack("dirt", 0, 3, 77);
    let source = screen(answered);
    assert!(runner.on_container_close(1.3, CONTAINER));
    assert_eq!(runner.request(Direction::Down, 2.0, false, &engine, &mut link), RotationResult::Started);
    assert!(runner.on_container_open(2.1, CONTAINER, INVENTORY));
    assert_eq!(runner.tick(2.2, true, Some(&source), &mut engine, &mut link), None);
    assert!(engine.busy());
    assert_eq!(link.sent.last(), Some(&Sent::Request));
}

#[test]
fn a_full_rotation_goes_out_as_one_request_packet() {
    let (mut link, mut engine) = (FakeLink::default(), engine());
    let mut full = model(0);
    for slot in 0..36u8 {
        full.player[usize::from(slot)] = stack("stone", i16::from(slot), 1, 100 + i32::from(slot));
    }
    let source = screen(full);
    let mut runner = opened(&mut link, &engine);

    assert_eq!(runner.tick(1.2, true, Some(&source), &mut engine, &mut link), None);

    assert_eq!(link.sent, [Sent::Open, Sent::Request]);
    // 9 columns of 3 swaps, one request each, in one payload whose first byte is the request count
    assert_eq!(link.requests[0][0], 27);
}
