use alaydriem_hotbar_manager::services::AnswerGate;
use alaydriem_hotbar_manager::traits::{InventoryLink, PacketSink};

const INVENTORY: i8 = -1;
const CHEST: i8 = 0;
const HIDDEN: i8 = 5;

// a link whose every send calls `inside` first, as a local server that answers while the packet is sent would
struct Answering<'a, F: FnMut(&str)> {
    inside: F,
    _gate: &'a AnswerGate,
}

impl<F: FnMut(&str)> PacketSink for Answering<'_, F> {
    fn send(&mut self, _bytes: &[u8]) -> bool {
        (self.inside)("request");
        true
    }
}

impl<F: FnMut(&str)> InventoryLink for Answering<'_, F> {
    fn open_inventory(&mut self) -> bool {
        (self.inside)("open");
        true
    }

    fn close_inventory(&mut self, _container_id: i8) -> bool {
        (self.inside)("close");
        true
    }
}

#[test]
fn a_busy_lock_with_no_send_in_progress_hides_nothing() {
    let gate = AnswerGate::new();

    assert!(!gate.busy_open(HIDDEN, INVENTORY));
    assert!(!gate.busy_close(HIDDEN));
    assert_eq!(gate.take_early_open(), None);
    assert_eq!(gate.take_early_close(), None);
}

#[test]
fn an_inventory_open_inside_the_interact_send_is_hidden_and_kept_for_the_runner() {
    let gate = AnswerGate::new();
    let mut hidden = None;
    let mut link = Answering {
        inside: |what: &str| {
            if what == "open" {
                hidden = Some(gate.busy_open(HIDDEN, INVENTORY))
            }
        },
        _gate: &gate,
    };

    gate.gated(&mut link).open_inventory();

    assert_eq!(hidden, Some(true));
    assert_eq!(gate.take_early_open(), Some(HIDDEN));
    assert_eq!(gate.take_early_open(), None);
}

#[test]
fn a_chest_open_inside_the_interact_send_is_not_hidden() {
    let gate = AnswerGate::new();
    let mut hidden = None;
    let mut link = Answering {
        inside: |what: &str| {
            if what == "open" {
                hidden = Some(gate.busy_open(HIDDEN, CHEST))
            }
        },
        _gate: &gate,
    };

    gate.gated(&mut link).open_inventory();

    assert_eq!(hidden, Some(false));
    assert_eq!(gate.take_early_open(), None);
}

#[test]
fn the_flag_is_armed_only_around_the_send() {
    let gate = AnswerGate::new();
    let mut link = Answering { inside: |_: &str| {}, _gate: &gate };
    gate.gated(&mut link).open_inventory();

    assert!(!gate.busy_open(HIDDEN, INVENTORY));
}

#[test]
fn a_request_send_does_not_arm_either_flag() {
    let gate = AnswerGate::new();
    let mut seen = Vec::new();
    let mut link = Answering {
        inside: |what: &str| {
            if what == "request" {
                seen.push(gate.busy_open(HIDDEN, INVENTORY));
                seen.push(gate.busy_close(HIDDEN));
            }
        },
        _gate: &gate,
    };

    gate.gated(&mut link).send(&[]);

    assert_eq!(seen, [false, false]);
}

#[test]
fn a_close_with_the_hidden_id_inside_the_close_send_is_hidden_once() {
    let gate = AnswerGate::new();
    let mut seen = Vec::new();
    let mut link = Answering {
        inside: |what: &str| {
            if what == "close" {
                seen.push(gate.busy_close(HIDDEN));
                seen.push(gate.busy_close(HIDDEN));
            }
        },
        _gate: &gate,
    };

    gate.gated(&mut link).close_inventory(HIDDEN);

    assert_eq!(seen, [true, false]);
    assert_eq!(gate.take_early_close(), Some(HIDDEN));
    assert_eq!(gate.take_early_close(), None);
}

#[test]
fn a_close_with_another_id_inside_the_close_send_is_not_hidden() {
    let gate = AnswerGate::new();
    let mut hidden = None;
    let mut link = Answering {
        inside: |what: &str| {
            if what == "close" {
                hidden = Some(gate.busy_close(HIDDEN + 1))
            }
        },
        _gate: &gate,
    };

    gate.gated(&mut link).close_inventory(HIDDEN);

    assert_eq!(hidden, Some(false));
    assert_eq!(gate.take_early_close(), None);
}

#[test]
fn a_close_after_the_close_send_is_not_hidden_by_the_gate() {
    let gate = AnswerGate::new();
    let mut link = Answering { inside: |_: &str| {}, _gate: &gate };
    gate.gated(&mut link).close_inventory(HIDDEN);

    assert!(!gate.busy_close(HIDDEN));
}
