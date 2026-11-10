use crate::structs::{ButtonAction, ScreenStep, Side, SlotAddr, SlotItem};

use super::SlotModel;

/// Main inventory rows: slots 9-35, not the hotbar.
pub const PLAYER_MAIN: [u8; 27] = {
    let mut slots = [0; 27];
    let mut i = 0;
    while i < 27 {
        slots[i] = 9 + i as u8;
        i += 1;
    }
    slots
};

/// Main inventory (9-35), then the hotbar (0-8): the order of move all, the filtered dumps and drop filtered.
pub const PLAYER_ALL: [u8; 36] = {
    let mut slots = [0; 36];
    let mut i = 0;
    while i < 36 {
        slots[i] = if i < 27 { 9 + i as u8 } else { (i - 27) as u8 };
        i += 1;
    }
    slots
};

/// The item a filtered button matches: the stack held on the cursor, by id and aux value only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFilter {
    pub id: String,
    pub aux: i16,
}

/// Turns a dump or drop button into the game's own handler calls. The plan holds one step for each
/// source slot; it does not check for free room, because the game moves what fits and leaves the rest.
pub struct StepPlanner;

impl StepPlanner {
    /// The steps of one button, in source order. `Sort` has none: `SortStepper` plans it.
    pub fn transfer(action: ButtonAction, model: &SlotModel, filter: Option<&ItemFilter>) -> Vec<ScreenStep> {
        match action {
            ButtonAction::Sort => Vec::new(),
            ButtonAction::DumpFromInventory => auto_place(model, Side::Player, &PLAYER_ALL, None),
            ButtonAction::DumpFromInventoryFiltered => auto_place(model, Side::Player, &PLAYER_ALL, filter),
            ButtonAction::DumpFromContainer => auto_place(model, Side::Container, &container_slots(model), None),
            ButtonAction::DumpFromContainerFiltered => {
                auto_place(model, Side::Container, &container_slots(model), filter)
            }
            ButtonAction::DropAll => drop_slots(model, &PLAYER_MAIN, None),
            ButtonAction::DropFiltered => {
                let mut steps = vec![ScreenStep::DropCursor];
                steps.extend(drop_slots(model, &PLAYER_ALL, filter));
                steps
            }
        }
    }
}

fn container_slots(model: &SlotModel) -> Vec<u8> {
    (0..=u8::MAX).take(model.container.len()).collect()
}

fn auto_place(model: &SlotModel, side: Side, slots: &[u8], filter: Option<&ItemFilter>) -> Vec<ScreenStep> {
    matching(model, side, slots, filter).map(ScreenStep::AutoPlace).collect()
}

fn drop_slots(model: &SlotModel, slots: &[u8], filter: Option<&ItemFilter>) -> Vec<ScreenStep> {
    matching(model, Side::Player, slots, filter).map(ScreenStep::DropSlot).collect()
}

/// The filled slots that pass the filter, in the order of `slots`.
fn matching<'a>(
    model: &'a SlotModel,
    side: Side,
    slots: &'a [u8],
    filter: Option<&'a ItemFilter>,
) -> impl Iterator<Item = SlotAddr> + 'a {
    slots.iter().filter_map(move |&slot| {
        let item = match side {
            Side::Container => model.container.get(usize::from(slot)),
            Side::Player => model.player.get(usize::from(slot)),
        }?
        .as_ref()?;
        passes(item, filter).then_some(SlotAddr { side, slot })
    })
}

/// A filtered button matches on item id and aux value only.
fn passes(item: &SlotItem, filter: Option<&ItemFilter>) -> bool {
    filter.is_none_or(|f| item.id == f.id && item.aux == f.aux)
}
