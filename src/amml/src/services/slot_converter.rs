use amml_api::ItemSnapshot;
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ScreenKind, SlotItem};

// Turns the stack snapshots of the loader into the slots the planners work on. A stack whose user data the loader
// cannot read has no `identity`: it gets a key of its own and is never merged. A snapshot that lacks the name, the aux
// or the largest stack size cannot be planned with, so a read with such a stack gives `None`
pub struct SlotConverter;

impl SlotConverter {
    pub const HOTBAR_SLOTS: usize = 9;
    pub const INVENTORY_SLOTS: usize = 27;
    pub const PLAYER_SLOTS: usize = 36;
    // the first byte of an unread key's hash input; no UTF-8 id (the first bytes of a plain key's input) starts with it
    const UNREAD_MARKER: u8 = 0xFF;

    // FNV-1a, 64 bit: a fixed hash, so a key is the same in every run
    pub fn fnv1a(bytes: &[u8]) -> u64 {
        bytes
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3))
    }

    // the key of a stack with a readable identity: the hash of the id, one zero byte (an id has none), the aux as two
    // little-endian bytes and the identity. The fixed-size parts before the identity keep two different triples from
    // making the same bytes
    pub fn stack_key(id: &str, aux: i16, identity: u64) -> u64 {
        let mut input = Vec::with_capacity(id.len() + 11);
        input.extend_from_slice(id.as_bytes());
        input.push(0);
        input.extend_from_slice(&aux.to_le_bytes());
        input.extend_from_slice(&identity.to_le_bytes());
        Self::fnv1a(&input)
    }

    // The key of a stack with no identity: the hash of the marker byte, the id and the aux. It holds no slot, so it
    // stays the same when the stack moves. Two such stacks of one id and aux share it, so the stack is also not
    // mergeable and Sort never merges them
    pub fn unread_key(id: &str, aux: i16) -> u64 {
        let mut input = Vec::with_capacity(id.len() + 4);
        input.push(Self::UNREAD_MARKER);
        input.extend_from_slice(id.as_bytes());
        input.push(0);
        input.extend_from_slice(&aux.to_le_bytes());
        Self::fnv1a(&input)
    }

    // one slot: `Some(None)` for an empty slot, `None` when a value the planners need is missing
    pub fn slot(snapshot: Option<&ItemSnapshot>) -> Option<Option<SlotItem>> {
        let Some(snapshot) = snapshot.filter(|snapshot| snapshot.count > 0) else { return Some(None) };
        let id = snapshot.type_name.clone()?;
        let aux = snapshot.aux?;
        let max = snapshot.max_count?;
        let (stack_key, mergeable) = match snapshot.identity {
            Some(identity) => (Self::stack_key(&id, aux, identity), true),
            None => (Self::unread_key(&id, aux), false),
        };
        Some(Some(SlotItem { id, aux, count: snapshot.count, max, stack_key, mergeable }))
    }

    // a list of slots; `None` when any slot cannot be read
    pub fn slots(snapshots: &[Option<ItemSnapshot>]) -> Option<Vec<Option<SlotItem>>> {
        snapshots.iter().map(|snapshot| Self::slot(snapshot.as_ref())).collect()
    }

    // the player's 36 slots (0..8 hotbar, 9..35 main inventory)
    pub fn player(snapshots: &[Option<ItemSnapshot>]) -> Option<[Option<SlotItem>; 36]> {
        Self::slots(snapshots)?.try_into().ok()
    }

    // the number of slots of the container the screen shows; 0 for the player inventory, None for another screen
    pub fn container_slots(kind: ScreenKind) -> Option<usize> {
        match kind {
            ScreenKind::SmallChest | ScreenKind::Barrel | ScreenKind::ShulkerBox | ScreenKind::EnderChest => Some(27),
            ScreenKind::LargeChest => Some(54),
            ScreenKind::Hopper => Some(5),
            ScreenKind::Dispenser | ScreenKind::Dropper => Some(9),
            ScreenKind::PlayerInventory => Some(0),
            ScreenKind::Other => None,
        }
    }

    // the model of the open screen from its three collections. `None` when a collection has another length than the
    // kind gives, or a slot cannot be read
    pub fn model(
        kind: ScreenKind,
        container: &[Option<ItemSnapshot>],
        hotbar: &[Option<ItemSnapshot>],
        inventory: &[Option<ItemSnapshot>],
    ) -> Option<SlotModel> {
        if Self::container_slots(kind)? != container.len()
            || hotbar.len() != Self::HOTBAR_SLOTS
            || inventory.len() != Self::INVENTORY_SLOTS
        {
            return None;
        }
        let player: Vec<Option<ItemSnapshot>> = hotbar.iter().chain(inventory).cloned().collect();
        Some(SlotModel { container: Self::slots(container)?, player: Self::player(&player)? })
    }
}
