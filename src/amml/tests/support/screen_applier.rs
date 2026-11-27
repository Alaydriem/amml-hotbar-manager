use amml_api::testing::{FakeItem, FakeScreen};

// What the game does to its slots for the container calls of a render, applied to the test's own screen between two
// renders (the fake changes only its copy of the screen inside one render). It knows the lines of
// `FakeGame::screen_actions`: take_all, place_all, swap, drop_slot and drop_cursor
pub struct ScreenApplier;

impl ScreenApplier {
    pub fn apply(screen: &mut FakeScreen, lines: &[String]) {
        for line in lines {
            let words: Vec<&str> = line.split(' ').collect();
            match words.as_slice() {
                ["take_all", collection, index] => Self::take_all(screen, collection, index),
                ["place_all", collection, index] => Self::place_all(screen, collection, index),
                ["swap", a, a_index, b, b_index] => Self::swap(screen, (a, a_index), (b, b_index)),
                ["drop_slot", collection, index, ..] => {
                    if let Some(cell) = Self::cell(screen, collection, index) {
                        *cell = None;
                    }
                }
                ["drop_cursor", ..] => screen.cursor = None,
                other => panic!("the applier does not know {other:?}"),
            }
        }
    }

    fn cell<'a>(screen: &'a mut FakeScreen, collection: &str, index: &str) -> Option<&'a mut Option<FakeItem>> {
        let index: usize = index.parse().ok()?;
        match collection {
            "container" => screen.slots.container.get_mut(index),
            "hotbar" => screen.slots.hotbar.get_mut(index),
            "inventory" => screen.slots.inventory.get_mut(index),
            _ => None,
        }
    }

    fn take_all(screen: &mut FakeScreen, collection: &str, index: &str) {
        if screen.cursor.is_some() {
            return;
        }
        let taken = Self::cell(screen, collection, index).and_then(Option::take);
        screen.cursor = taken;
    }

    fn place_all(screen: &mut FakeScreen, collection: &str, index: &str) {
        let held = screen.cursor.take();
        let Some(cell) = Self::cell(screen, collection, index) else {
            screen.cursor = held;
            return;
        };
        match (cell.take(), held) {
            (None, held) => *cell = held,
            (Some(mut there), Some(mut held))
                if there.type_name == held.type_name && there.identity == held.identity =>
            {
                let room = there.max_count.unwrap_or(64).saturating_sub(there.count);
                let moved = held.count.min(room);
                there.count += moved;
                held.count -= moved;
                *cell = Some(there);
                screen.cursor = (held.count > 0).then_some(held);
            }
            (Some(there), Some(held)) => {
                *cell = Some(held);
                screen.cursor = Some(there);
            }
            (there, None) => *cell = there,
        }
    }

    fn swap(screen: &mut FakeScreen, a: (&str, &str), b: (&str, &str)) {
        let first = Self::cell(screen, a.0, a.1).map(|cell| *cell);
        let second = Self::cell(screen, b.0, b.1).map(|cell| *cell);
        let (Some(first), Some(second)) = (first, second) else { return };
        if let Some(cell) = Self::cell(screen, a.0, a.1) {
            *cell = second;
        }
        if let Some(cell) = Self::cell(screen, b.0, b.1) {
            *cell = first;
        }
    }
}
