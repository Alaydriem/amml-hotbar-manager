// Tells whether a screen is open. The game gives no close signal to the mod, so the last view that drew its buttons is
// remembered with the time it did: a view that drew within `OPEN_SECONDS` is open, and when it stops drawing the
// screen has closed or changed. Both views of a container screen draw alternately, and either one gives the same
// controller, so the newest of them is kept
#[derive(Debug, Default)]
pub struct ScreenWatch {
    seen: Option<(usize, f64)>,
}

impl ScreenWatch {
    // the longest gap between two frames of an open screen
    pub const OPEN_SECONDS: f64 = 0.15;

    pub fn new() -> Self {
        Self::default()
    }

    // `view` drew its buttons at `now`
    pub fn seen(&mut self, view: usize, now: f64) {
        self.seen = Some((view, now));
    }

    // the view of the open screen at `now`; None when no view drew lately
    pub fn open_view(&self, now: f64) -> Option<usize> {
        self.seen.filter(|(_, at)| now - at <= Self::OPEN_SECONDS).map(|(view, _)| view)
    }

    // the mod is disabled: no screen is open
    pub fn clear(&mut self) {
        self.seen = None;
    }
}
