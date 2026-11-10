use alaydriem_hotbar_manager::game::{RequestEncoder, SlotRef};
use alaydriem_hotbar_manager::services::MoveEngine;
use alaydriem_hotbar_manager::types::{MoveEnd, MoveOutcome};
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ItemMove, ScreenKind, Side, SlotAddr, SlotItem};

use crate::support::fake_sender::{FakePredictor, FakeSender};

const FIRST: i32 = MoveEngine::<FakePredictor>::FIRST_REQUEST_ID;
const SECOND: i32 = FIRST - 2;
const THIRD: i32 = FIRST - 4;

fn item(key: u64, net_id: i32, count: u8) -> Option<SlotItem> {
    Some(SlotItem {
        id: format!("minecraft:item_{key}"),
        aux: 0,
        count,
        max: 64,
        stack_key: key,
        net_id,
        mergeable: true,
    })
}

fn chest(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Container, slot }
}

fn player(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Player, slot }
}

// chest 0: 10 of item 1 (net 11); chest 1: 20 of item 2 (net 12); chest 2: 30 of item 3 (net 13); hotbar 0: 5 of
// item 4 (net 14); main 9: 7 of item 1 (net 15)
fn model() -> SlotModel {
    let mut model = SlotModel { container: vec![None; 27], player: std::array::from_fn(|_| None) };
    model.container[0] = item(1, 11, 10);
    model.container[1] = item(2, 12, 20);
    model.container[2] = item(3, 13, 30);
    model.player[0] = item(4, 14, 5);
    model.player[9] = item(1, 15, 7);
    model
}

fn engine() -> MoveEngine<FakePredictor> {
    MoveEngine::new(FakePredictor::default())
}

fn applied(model: &SlotModel, moves: &[ItemMove]) -> SlotModel {
    let mut model = model.clone();
    for mv in moves {
        model.apply(mv).unwrap();
    }
    model
}

fn three_moves() -> Vec<ItemMove> {
    vec![
        ItemMove::Place { from: chest(0), to: player(1), count: 10 },
        ItemMove::Swap { a: chest(1), b: player(0) },
        ItemMove::Place { from: chest(2), to: player(2), count: 30 },
    ]
}

#[test]
fn sends_moves_in_order_one_at_a_time() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = vec![
        ItemMove::Place { from: chest(0), to: player(1), count: 10 },
        ItemMove::Swap { a: chest(1), b: player(0) },
    ];

    assert!(engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest));
    engine.tick(0.0, &snapshot, &mut sink);
    engine.tick(0.1, &snapshot, &mut sink);

    // only the first request goes out until its answer comes
    let place = RequestEncoder::place(
        FIRST,
        10,
        SlotRef { container: 7, slot: 0, net_id: 11 },
        SlotRef { container: SlotRef::HOTBAR, slot: 1, net_id: 0 },
    );
    assert_eq!(sink.sent, vec![place.clone()]);
    assert!(engine.busy());

    engine.on_response(FIRST, true);
    let live = applied(&snapshot, &moves[..1]);
    engine.tick(0.2, &live, &mut sink);

    let swap = RequestEncoder::swap(
        SECOND,
        SlotRef { container: 7, slot: 1, net_id: 12 },
        SlotRef { container: SlotRef::HOTBAR, slot: 0, net_id: 14 },
    );
    assert_eq!(sink.sent, vec![place, swap]);

    engine.on_response(SECOND, true);
    engine.tick(0.3, &applied(&snapshot, &moves), &mut sink);

    assert!(!engine.busy());
    assert_eq!(sink.sent.len(), 2);
    assert_eq!(engine.take_outcome(), Some(MoveOutcome { accepted: 2, total: 2, end: MoveEnd::Done }));
    assert_eq!(engine.take_outcome(), None);
}

#[test]
fn rejected_request_stops_action_and_reports_undo() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = three_moves();

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(FIRST, true);
    let live = applied(&snapshot, &moves[..1]);
    engine.tick(0.1, &live, &mut sink);
    engine.on_response(SECOND, false);
    engine.tick(0.2, &live, &mut sink);
    engine.tick(0.3, &live, &mut sink);

    // the third move is dropped; the rejected one is named, with the count the server accepted
    assert_eq!(sink.sent.len(), 2);
    assert!(!engine.busy());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome {
            accepted: 1,
            total: 3,
            end: MoveEnd::Rejected { mv: moves[1], index: 1, timed_out: false, undone: true },
        })
    );
    // the prediction of the rejected move is put back; the accepted one stays
    let predictor = engine.predictor();
    assert_eq!(predictor.undone, vec![moves[1]]);
    assert_eq!(predictor.confirmed, vec![moves[0]]);
}

#[test]
fn timeout_counts_as_rejected() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = three_moves();

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.tick(MoveEngine::<FakePredictor>::REQUEST_TIMEOUT_SECONDS - 0.01, &snapshot, &mut sink);
    assert!(engine.busy());
    assert_eq!(engine.take_outcome(), None);

    engine.tick(MoveEngine::<FakePredictor>::REQUEST_TIMEOUT_SECONDS, &snapshot, &mut sink);

    assert!(!engine.busy());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome {
            accepted: 0,
            total: 3,
            end: MoveEnd::Rejected { mv: moves[0], index: 0, timed_out: true, undone: true },
        })
    );
    assert_eq!(engine.predictor().undone, vec![moves[0]]);

    // a late answer changes nothing, and no more requests go out
    engine.on_response(FIRST, true);
    engine.tick(1.0, &snapshot, &mut sink);
    assert_eq!(sink.sent.len(), 1);
    assert_eq!(engine.take_outcome(), None);
    assert!(engine.predictor().confirmed.is_empty());
}

#[test]
fn stale_source_net_id_stops_action() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = three_moves();
    // a hopper or another player replaced the stack in chest 0 after the plan was made
    let mut live = snapshot.clone();
    live.container[0] = item(1, 99, 10);

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &live, &mut sink);
    // the live view gets a short time to catch up before the action stops
    assert!(engine.busy());
    engine.tick(MoveEngine::<FakePredictor>::SETTLE_SECONDS, &live, &mut sink);

    assert!(sink.sent.is_empty());
    assert!(!engine.busy());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome { accepted: 0, total: 3, end: MoveEnd::Stale { mv: moves[0], index: 0 } })
    );
}

#[test]
fn stale_source_after_an_accepted_move_stops_action() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = three_moves();

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(FIRST, true);
    let mut live = applied(&snapshot, &moves[..1]);
    live.container[1] = item(2, 77, 20);
    engine.tick(1.0, &live, &mut sink);
    engine.tick(1.0 + MoveEngine::<FakePredictor>::SETTLE_SECONDS, &live, &mut sink);

    assert_eq!(sink.sent.len(), 1);
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome { accepted: 1, total: 3, end: MoveEnd::Stale { mv: moves[1], index: 1 } })
    );
}

#[test]
fn a_stack_put_into_the_target_slot_stops_action() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = vec![ItemMove::Place { from: chest(0), to: player(1), count: 10 }];
    let mut live = snapshot.clone();
    live.player[1] = item(5, 40, 1);

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(MoveEngine::<FakePredictor>::SETTLE_SECONDS, &live, &mut sink);

    assert!(sink.sent.is_empty());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome { accepted: 0, total: 1, end: MoveEnd::Stale { mv: moves[0], index: 0 } })
    );
}

#[test]
fn a_live_view_that_catches_up_within_the_settle_time_goes_on() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    // the second move takes the stack the first one put into hotbar 1
    let moves = vec![
        ItemMove::Place { from: chest(0), to: player(1), count: 10 },
        ItemMove::Place { from: player(1), to: player(2), count: 10 },
    ];

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(FIRST, true);
    // the game shows the accepted move one tick late
    engine.tick(0.05, &snapshot, &mut sink);
    assert_eq!(sink.sent.len(), 1);
    engine.tick(0.1, &applied(&snapshot, &moves[..1]), &mut sink);

    assert_eq!(sink.sent.len(), 2);
    assert!(engine.busy());
}

#[test]
fn second_start_while_busy_is_ignored() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let first = vec![ItemMove::Place { from: chest(0), to: player(1), count: 10 }];
    let second = vec![ItemMove::Place { from: chest(2), to: player(2), count: 30 }];

    assert!(engine.start(0.0, first.clone(), &snapshot, ScreenKind::SmallChest));
    assert!(!engine.start(0.0, second, &snapshot, ScreenKind::SmallChest));
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(FIRST, true);
    engine.tick(0.1, &applied(&snapshot, &first), &mut sink);

    assert_eq!(sink.sent.len(), 1);
    assert_eq!(engine.take_outcome(), Some(MoveOutcome { accepted: 1, total: 1, end: MoveEnd::Done }));
    assert!(engine.start(0.2, vec![], &snapshot, ScreenKind::SmallChest));
}

#[test]
fn cancel_clears_queue() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = three_moves();

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.cancel();

    assert!(!engine.busy());
    assert_eq!(engine.take_outcome(), Some(MoveOutcome { accepted: 0, total: 3, end: MoveEnd::Cancelled }));
    // the prediction of the request in flight is put back, a late answer is ignored, and nothing more is sent
    assert_eq!(engine.predictor().undone, vec![moves[0]]);
    engine.on_response(FIRST, true);
    engine.tick(0.1, &snapshot, &mut sink);
    assert_eq!(sink.sent.len(), 1);
    assert_eq!(engine.take_outcome(), None);

    // a new action starts, with a fresh request id
    assert!(engine.start(0.2, moves[2..].to_vec(), &snapshot, ScreenKind::SmallChest));
    engine.tick(0.2, &snapshot, &mut sink);
    let place = RequestEncoder::place(
        SECOND,
        30,
        SlotRef { container: 7, slot: 2, net_id: 13 },
        SlotRef { container: SlotRef::HOTBAR, slot: 2, net_id: 0 },
    );
    assert_eq!(sink.sent.last(), Some(&place));
}

#[test]
fn place_onto_a_stack_writes_the_target_net_id_and_drop_writes_a_drop() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = vec![
        ItemMove::Place { from: chest(0), to: player(9), count: 10 },
        ItemMove::Drop { from: chest(2), count: 30 },
    ];

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::Barrel);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(FIRST, true);
    engine.tick(0.1, &applied(&snapshot, &moves[..1]), &mut sink);

    let place = RequestEncoder::place_onto(
        FIRST,
        10,
        SlotRef { container: 58, slot: 0, net_id: 11 },
        SlotRef { container: SlotRef::INVENTORY, slot: 9, net_id: 15 },
    );
    let drop = RequestEncoder::drop(SECOND, 30, SlotRef { container: 58, slot: 2, net_id: 13 });
    assert_eq!(sink.sent, vec![place, drop]);
}

#[test]
fn a_split_stack_takes_the_net_id_the_server_gives_it() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    // half of chest 2 into hotbar 3, then that new stack on to hotbar 4
    let moves = vec![
        ItemMove::Place { from: chest(2), to: player(3), count: 15 },
        ItemMove::Place { from: player(3), to: player(4), count: 15 },
    ];

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::ShulkerBox);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(FIRST, true);
    let mut live = applied(&snapshot, &moves[..1]);
    live.player[3] = item(3, 50, 15);
    engine.tick(0.1, &live, &mut sink);

    let second = RequestEncoder::place(
        SECOND,
        15,
        SlotRef { container: SlotRef::HOTBAR, slot: 3, net_id: 50 },
        SlotRef { container: SlotRef::HOTBAR, slot: 4, net_id: 0 },
    );
    assert_eq!(sink.sent.last(), Some(&second));
}

#[test]
fn a_container_move_on_a_screen_without_a_container_id_is_not_sent() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = vec![ItemMove::Place { from: chest(0), to: player(1), count: 10 }];

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::PlayerInventory);
    engine.tick(0.0, &snapshot, &mut sink);

    assert!(sink.sent.is_empty());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome { accepted: 0, total: 1, end: MoveEnd::NotSent { mv: moves[0], index: 0 } })
    );
}

#[test]
fn a_request_the_sink_cannot_send_stops_action() {
    let mut engine = engine();
    let mut sink = FakeSender { refuse: true, ..FakeSender::default() };
    let snapshot = model();
    let moves = three_moves();

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);

    assert!(!engine.busy());
    assert!(engine.predictor().predicted.is_empty());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome { accepted: 0, total: 3, end: MoveEnd::NotSent { mv: moves[0], index: 0 } })
    );
}

#[test]
fn a_rejection_without_a_prediction_reports_nothing_undone() {
    let mut engine = MoveEngine::new(FakePredictor { skip: true, ..FakePredictor::default() });
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = three_moves();

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(FIRST, false);

    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome {
            accepted: 0,
            total: 3,
            end: MoveEnd::Rejected { mv: moves[0], index: 0, timed_out: false, undone: false },
        })
    );
    assert!(engine.predictor().undone.is_empty());
}

#[test]
fn an_answer_to_another_request_is_ignored() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();

    engine.start(0.0, three_moves(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    engine.on_response(-3, false);
    engine.on_response(THIRD, false);

    assert!(engine.busy());
    assert_eq!(engine.take_outcome(), None);
}

#[test]
fn outcomes_other_than_done_ask_for_a_resync() {
    let done = MoveOutcome { accepted: 1, total: 1, end: MoveEnd::Done };
    let cancelled = MoveOutcome { accepted: 0, total: 1, end: MoveEnd::Cancelled };

    assert!(!done.needs_resync());
    assert!(cancelled.needs_resync());
}

#[test]
fn a_changed_count_with_the_same_key_and_net_id_stops_action() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    let moves = vec![ItemMove::Place { from: chest(0), to: player(9), count: 10 }];
    // a hopper added one item to chest 0: same key, same net id, one more item
    let mut live = snapshot.clone();
    live.container[0] = item(1, 11, 11);

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &live, &mut sink);
    engine.tick(MoveEngine::<FakePredictor>::SETTLE_SECONDS, &live, &mut sink);

    assert!(sink.sent.is_empty());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome { accepted: 0, total: 1, end: MoveEnd::Stale { mv: moves[0], index: 0 } })
    );
}

#[test]
fn an_accepted_answer_starts_the_settle_time_again() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    // the second move takes the stack the first one put into hotbar 1
    let moves = vec![
        ItemMove::Place { from: chest(0), to: player(1), count: 10 },
        ItemMove::Place { from: player(1), to: player(2), count: 10 },
    ];
    let settle = MoveEngine::<FakePredictor>::SETTLE_SECONDS;

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);
    // the answer comes after the settle time of `start` has run out
    engine.tick(settle - 0.1, &snapshot, &mut sink);
    engine.on_response(FIRST, true);
    // the live view lags one tick behind the accepted move, then catches up
    engine.tick(settle + 0.1, &snapshot, &mut sink);
    assert!(engine.busy());
    assert_eq!(engine.take_outcome(), None);
    engine.tick(settle + 0.15, &applied(&snapshot, &moves[..1]), &mut sink);

    assert_eq!(sink.sent.len(), 2);
}

#[test]
fn a_move_the_expected_model_refuses_is_not_sent() {
    let mut engine = engine();
    let mut sink = FakeSender::default();
    let snapshot = model();
    // a planner bug: a swap with an empty side
    let moves = vec![ItemMove::Swap { a: chest(0), b: player(5) }];

    engine.start(0.0, moves.clone(), &snapshot, ScreenKind::SmallChest);
    engine.tick(0.0, &snapshot, &mut sink);

    assert!(sink.sent.is_empty());
    assert_eq!(
        engine.take_outcome(),
        Some(MoveOutcome { accepted: 0, total: 1, end: MoveEnd::NotSent { mv: moves[0], index: 0 } })
    );
}
