use std::collections::VecDeque;

use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ItemMove, ScreenKind, Side, SlotAddr, SlotItem};

use crate::game::{ContainerIds, RequestAction, RequestEncoder, SlotRef};
use crate::traits::{MovePredictor, PacketSink};
use crate::types::{MoveEnd, MoveOutcome};

// a batch: every move of an action in one packet, each its own request. Before the send `sent_at` is None. A batch
// cancelled after its send is `detached`: its end was reported, but the server still does its moves, so it is kept
// until its answers or its time is up and then settled without a second report
#[derive(Debug, Clone)]
struct Batch {
    moves: Vec<ItemMove>,
    ids: Vec<i32>,
    answers: Vec<Option<bool>>,
    predicted: Vec<bool>,
    sent_at: Option<f64>,
    detached: bool,
}

// the request in flight: its id, its move, when it was sent, and whether the client shows it already
#[derive(Debug, Clone, Copy)]
struct InFlight {
    id: i32,
    mv: ItemMove,
    sent_at: f64,
    predicted: bool,
}

// Sends the planned moves of one action (a sort, a dump, a drop) as vanilla item stack requests, one request in
// flight at a time, the way stratum sends its requests. Each move is one request with one action. Before a move is
// sent, the slots it uses must hold what the expected model says (the snapshot of `start`, with each accepted move
// applied): the same stack key, count and net id, and an empty target must be empty. A mismatch that lasts
// SETTLE_SECONDS stops the action as stale. A refusal, or no answer within REQUEST_TIMEOUT_SECONDS, stops the action
// and undoes the client prediction of that move. Each stop is reported once through `take_outcome`, so that the
// caller can read the slots again. `start_batch` instead sends every move at once, in one packet of one request per
// move (the row rotation): the client shows them all at once, and a refused move leaves the moves the server accepted
// done, with the client display put back in step with them. A pure state machine: the game is reached only through the sink and the predictor
pub struct MoveEngine<P: MovePredictor> {
    predictor: P,
    queue: VecDeque<ItemMove>,
    expected: SlotModel,
    container_id: Option<u8>,
    total: usize,
    accepted: usize,
    in_flight: Option<InFlight>,
    batch: Option<Batch>,
    // when the expected model last changed (start, or an accepted move, seen at the next tick)
    settle_from: Option<f64>,
    next_id: i32,
    outcome: Option<MoveOutcome>,
}

impl<P: MovePredictor> MoveEngine<P> {
    // request ids are negative and odd, the way the game's own client ids are, and far from them: vanilla counts
    // -1, -3, ... per process. A step of 2 keeps them odd
    pub const FIRST_REQUEST_ID: i32 = -100001;
    const REQUEST_ID_STEP: i32 = 2;
    // the wait for an answer: stratum's REQUEST_TIMEOUT_SECONDS
    pub const REQUEST_TIMEOUT_SECONDS: f64 = 0.5;
    // how long the live slots may lag behind the expected model before the action is stale
    pub const SETTLE_SECONDS: f64 = 0.5;
    // the wait for every answer of a batch, one window for the whole packet: RotationRunner::ANSWER_WINDOW_SECONDS
    pub const BATCH_TIMEOUT_SECONDS: f64 = 2.0;

    pub fn new(predictor: P) -> Self {
        Self {
            predictor,
            queue: VecDeque::new(),
            expected: SlotModel { container: Vec::new(), player: std::array::from_fn(|_| None) },
            container_id: None,
            total: 0,
            accepted: 0,
            in_flight: None,
            batch: None,
            settle_from: None,
            next_id: Self::FIRST_REQUEST_ID,
            outcome: None,
        }
    }

    // starts an action: `moves` in order, planned on `snapshot`, on a screen of `kind`. False, changing nothing,
    // while an action runs. An empty plan is Done at once
    pub fn start(&mut self, now: f64, moves: Vec<ItemMove>, snapshot: &SlotModel, kind: ScreenKind) -> bool {
        if self.busy() {
            return false;
        }
        self.expected = snapshot.clone();
        self.container_id = ContainerIds::for_screen(kind);
        self.total = moves.len();
        self.accepted = 0;
        self.queue = moves.into();
        self.settle_from = Some(now);
        self.outcome = None;
        if self.queue.is_empty() {
            self.finish(MoveEnd::Done);
        }
        true
    }

    // starts an action whose moves go out at once, in one packet (see the type's comment). Same contract as `start`
    pub fn start_batch(&mut self, now: f64, moves: Vec<ItemMove>, snapshot: &SlotModel, kind: ScreenKind) -> bool {
        if moves.is_empty() || self.busy() {
            return self.start(now, moves, snapshot, kind);
        }
        self.start(now, Vec::new(), snapshot, kind);
        self.total = moves.len();
        self.outcome = None;
        self.batch = Some(Batch {
            moves,
            ids: Vec::new(),
            answers: Vec::new(),
            predicted: Vec::new(),
            sent_at: None,
            detached: false,
        });
        true
    }

    // ends a sent batch whose answers did not all come within BATCH_TIMEOUT_SECONDS (a missing answer counts as
    // refused). The rotation calls it on every tick, so a cancelled batch ends too
    pub fn expire(&mut self, now: f64) {
        let Some(batch) = self.batch.as_ref() else { return };
        let Some(sent_at) = batch.sent_at else { return };
        if now - sent_at >= Self::BATCH_TIMEOUT_SECONDS {
            curia::info!("item stack batch not fully answered", { requests: batch.ids.len() as i64 });
            self.settle_batch();
        }
    }

    // once per client tick, with the slots the client shows now: ends a request that waited too long, or sends the
    // next move when nothing is in flight
    pub fn tick(&mut self, now: f64, live: &SlotModel, sink: &mut dyn PacketSink) {
        if self.batch.is_some() {
            self.tick_batch(now, live, sink);
            return;
        }
        if let Some(flight) = self.in_flight {
            if now - flight.sent_at >= Self::REQUEST_TIMEOUT_SECONDS {
                curia::info!("item stack request not answered", { id: flight.id });
                self.reject(flight, true);
            }
            return;
        }
        let Some(&mv) = self.queue.front() else { return };
        let index = self.accepted;
        let settle_from = *self.settle_from.get_or_insert(now);
        if self.expected.clone().apply(&mv).is_err() {
            self.finish(MoveEnd::NotSent { mv, index });
            return;
        }
        if !self.matches(live, &mv) {
            if now - settle_from >= Self::SETTLE_SECONDS {
                self.finish(MoveEnd::Stale { mv, index });
            }
            return;
        }
        let id = self.next_id();
        let sent = self.encode(id, &mv, live).is_some_and(|bytes| sink.send(&bytes));
        if !sent {
            self.finish(MoveEnd::NotSent { mv, index });
            return;
        }
        let predicted = self.predictor.predict(&mv, &self.expected);
        self.queue.pop_front();
        self.in_flight = Some(InFlight { id, mv, sent_at: now, predicted });
    }

    // the server's answer to request `request_id`; answers to other requests are ignored
    pub fn on_response(&mut self, request_id: i32, accepted: bool) {
        if let Some(batch) = self.batch.as_mut() {
            let Some(index) = batch.ids.iter().position(|id| *id == request_id) else { return };
            batch.answers[index].get_or_insert(accepted);
            if batch.answers.iter().all(Option::is_some) {
                self.settle_batch();
            }
            return;
        }
        let Some(flight) = self.in_flight.filter(|flight| flight.id == request_id) else { return };
        if !accepted {
            curia::info!("item stack request refused", { id: request_id });
            self.reject(flight, false);
            return;
        }
        self.in_flight = None;
        self.predictor.confirm(&flight.mv);
        self.apply_expected(&flight.mv);
        self.accepted += 1;
        self.settle_from = None;
        if self.queue.is_empty() {
            self.finish(MoveEnd::Done);
        }
    }

    // drops the action: the queue, and the request in flight with its prediction undone. Answers that come later are
    // ignored. A batch already sent cannot be called back: the end is reported now, but the batch stays (busy) and is
    // settled on its answers or by `expire`, so the client shows what the server did. Nothing happens while no action
    // runs
    pub fn cancel(&mut self) {
        if !self.busy() {
            return;
        }
        if let Some(batch) = self.batch.as_mut()
            && batch.sent_at.is_some()
        {
            if !batch.detached {
                batch.detached = true;
                self.outcome =
                    Some(MoveOutcome { accepted: self.accepted, total: self.total, end: MoveEnd::Cancelled });
            }
            return;
        }
        if let Some(flight) = self.in_flight
            && flight.predicted
        {
            self.predictor.undo(&flight.mv);
        }
        self.finish(MoveEnd::Cancelled);
    }

    pub fn busy(&self) -> bool {
        self.in_flight.is_some() || !self.queue.is_empty() || self.batch.is_some()
    }

    // how the last action ended, once
    pub fn take_outcome(&mut self) -> Option<MoveOutcome> {
        self.outcome.take()
    }

    // the snapshot of `start` with every accepted move applied
    pub fn expected(&self) -> &SlotModel {
        &self.expected
    }

    pub fn predictor(&self) -> &P {
        &self.predictor
    }

    pub fn predictor_mut(&mut self) -> &mut P {
        &mut self.predictor
    }

    fn reject(&mut self, flight: InFlight, timed_out: bool) {
        let undone = flight.predicted && self.predictor.undo(&flight.mv);
        let end = MoveEnd::Rejected { mv: flight.mv, index: self.accepted, timed_out, undone };
        self.finish(end);
    }

    fn finish(&mut self, end: MoveEnd) {
        self.queue.clear();
        self.in_flight = None;
        self.batch = None;
        self.outcome = Some(MoveOutcome { accepted: self.accepted, total: self.total, end });
    }

    // a batch before its send waits for the live slots to hold the snapshot, then sends every move in one packet and
    // shows them all; after the send it ends the batch once the time for the answers is up
    fn tick_batch(&mut self, now: f64, live: &SlotModel, sink: &mut dyn PacketSink) {
        let Some(batch) = self.batch.as_ref() else { return };
        if batch.sent_at.is_some() {
            self.expire(now);
            return;
        }
        let moves = batch.moves.clone();
        let settle_from = *self.settle_from.get_or_insert(now);
        if !moves.iter().all(|mv| self.matches(live, mv)) {
            if now - settle_from >= Self::SETTLE_SECONDS {
                self.finish(MoveEnd::Stale { mv: moves[0], index: 0 });
            }
            return;
        }
        // each request names the net ids its slots hold once the moves before it are done
        let mut after = self.expected.clone();
        let mut requests = Vec::with_capacity(moves.len());
        for (index, mv) in moves.iter().enumerate() {
            let action = self.action(mv, &after);
            let (Some(action), Ok(())) = (action, after.apply(mv)) else {
                self.finish(MoveEnd::NotSent { mv: *mv, index });
                return;
            };
            requests.push((self.next_id(), action));
        }
        if !sink.send(&RequestEncoder::batch(&requests)) {
            self.finish(MoveEnd::NotSent { mv: moves[0], index: 0 });
            return;
        }
        let mut before = self.expected.clone();
        let mut predicted = Vec::with_capacity(moves.len());
        for mv in &moves {
            predicted.push(self.predictor.predict(mv, &before));
            // every move applied above
            let _ = before.apply(mv);
        }
        if let Some(batch) = self.batch.as_mut() {
            batch.ids = requests.iter().map(|(id, _)| *id).collect();
            batch.answers = vec![None; moves.len()];
            batch.predicted = predicted;
            batch.sent_at = Some(now);
        }
    }

    // ends a sent batch; a missing answer counts as refused. All accepted, the predictions are confirmed. Otherwise
    // every prediction is undone (newest first) and the accepted moves are shown again in order, so that the client
    // shows what the server did
    fn settle_batch(&mut self) {
        let Some(batch) = self.batch.take() else { return };
        let Some(refused) = batch.answers.iter().position(|answer| *answer != Some(true)) else {
            for (mv, predicted) in batch.moves.iter().zip(&batch.predicted) {
                if *predicted {
                    self.predictor.confirm(mv);
                }
                self.apply_expected(mv);
            }
            self.accepted = batch.moves.len();
            self.end_batch(&batch, MoveEnd::Done);
            return;
        };
        let timed_out = batch.answers.iter().any(Option::is_none);
        let undone = self.undo_batch(&batch);
        for (mv, answer) in batch.moves.iter().zip(&batch.answers) {
            if *answer != Some(true) {
                continue;
            }
            let before = self.expected.clone();
            if before.clone().apply(mv).is_err() {
                break;
            }
            // the server did this move: shown again, with no answer owed for it
            if self.predictor.predict(mv, &before) {
                self.predictor.confirm(mv);
            }
            self.apply_expected(mv);
            self.accepted += 1;
        }
        let mv = batch.moves[refused];
        self.end_batch(&batch, MoveEnd::Rejected { mv, index: refused, timed_out, undone });
    }

    // the end of a settled batch: reported, unless a cancel reported it already
    fn end_batch(&mut self, batch: &Batch, end: MoveEnd) {
        if batch.detached {
            if end != MoveEnd::Done {
                curia::info!("a cancelled item stack batch was not fully accepted", {
                    accepted: self.accepted as i64,
                    total: self.total as i64
                });
            }
            return;
        }
        self.finish(end);
    }

    // puts back every prediction of `batch`, newest first; true when something was put back
    fn undo_batch(&mut self, batch: &Batch) -> bool {
        let mut undone = false;
        for (mv, predicted) in batch.moves.iter().zip(&batch.predicted).rev() {
            if *predicted {
                undone |= self.predictor.undo(mv);
            }
        }
        undone
    }

    fn next_id(&mut self) -> i32 {
        let id = self.next_id;
        self.next_id -= Self::REQUEST_ID_STEP;
        // starts again long before the id leaves the int range
        if self.next_id > 0 || self.next_id < Self::FIRST_REQUEST_ID * 1000 {
            self.next_id = Self::FIRST_REQUEST_ID;
        }
        id
    }

    // the accepted move on the expected model. Part of a stack placed into an empty slot is a new stack whose net id
    // the server gives, so its expected net id becomes 0 (unknown) and is not checked
    fn apply_expected(&mut self, mv: &ItemMove) {
        let split = match *mv {
            ItemMove::Place { from, to, count } => {
                let whole = Self::slot(&self.expected, from).is_some_and(|source| source.count == count);
                let empty = Self::slot(&self.expected, to).is_none();
                (empty && !whole).then_some(to)
            }
            _ => None,
        };
        if self.expected.apply(mv).is_err() {
            return;
        }
        if let Some(to) = split
            && let Some(Some(item)) = Self::slot_mut(&mut self.expected, to)
        {
            item.net_id = 0;
        }
    }

    // whether every slot `mv` uses holds on `live` what the expected model says
    fn matches(&self, live: &SlotModel, mv: &ItemMove) -> bool {
        let same = |addr: SlotAddr| Self::same(Self::slot(&self.expected, addr), Self::slot(live, addr));
        match *mv {
            ItemMove::Swap { a, b } => same(a) && same(b),
            ItemMove::Place { from, to, .. } => same(from) && same(to),
            ItemMove::Drop { from, .. } => same(from),
        }
    }

    // an empty slot matches an empty slot; a stack matches a stack of the same key and count, and the same net id
    // unless the expected one is unknown (0)
    fn same(expected: Option<&SlotItem>, live: Option<&SlotItem>) -> bool {
        match (expected, live) {
            (None, None) => true,
            (Some(expected), Some(live)) => {
                expected.stack_key == live.stack_key
                    && expected.count == live.count
                    && (expected.net_id == 0 || expected.net_id == live.net_id)
            }
            _ => false,
        }
    }

    // the request for `mv`, named with the net ids the client shows now; None when a slot has no container id
    fn encode(&self, id: i32, mv: &ItemMove, live: &SlotModel) -> Option<Vec<u8>> {
        Some(RequestEncoder::batch(&[(id, self.action(mv, live)?)]))
    }

    // the action of `mv`, named with the net ids of `model`; None when a slot has no container id
    fn action(&self, mv: &ItemMove, model: &SlotModel) -> Option<RequestAction> {
        let slot = |addr: SlotAddr| self.slot_ref(addr, model);
        Some(match *mv {
            ItemMove::Swap { a, b } => RequestAction::Swap { from: slot(a)?, to: slot(b)? },
            ItemMove::Place { from, to, count } if Self::slot(model, to).is_none() => {
                RequestAction::Place { count, from: slot(from)?, to: slot(to)? }
            }
            ItemMove::Place { from, to, count } => RequestAction::PlaceOnto { count, from: slot(from)?, to: slot(to)? },
            ItemMove::Drop { from, count } => RequestAction::Drop { count, from: slot(from)? },
        })
    }

    fn slot_ref(&self, addr: SlotAddr, live: &SlotModel) -> Option<SlotRef> {
        let net_id = Self::slot(live, addr).map_or(0, |item| item.net_id);
        let (container, slot) = match addr.side {
            Side::Container => (self.container_id?, addr.slot),
            Side::Player => ContainerIds::player(addr.slot),
        };
        Some(SlotRef { container, slot, net_id })
    }

    fn slot(model: &SlotModel, addr: SlotAddr) -> Option<&SlotItem> {
        let index = usize::from(addr.slot);
        match addr.side {
            Side::Container => model.container.get(index),
            Side::Player => model.player.get(index),
        }
        .and_then(Option::as_ref)
    }

    fn slot_mut(model: &mut SlotModel, addr: SlotAddr) -> Option<&mut Option<SlotItem>> {
        let index = usize::from(addr.slot);
        match addr.side {
            Side::Container => model.container.get_mut(index),
            Side::Player => model.player.get_mut(index),
        }
    }
}
