use std::ffi::c_void;

use amml_game::{
    ContainerScreenController, ItemInstanceLayout, ItemStackBaseRef, ItemStackNetIdVariantLayout, RawObject,
};
use amml_msvc::{MsvcHeap, MsvcString};
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ScreenKind, SlotItem};

use crate::traits::GameHost;
use crate::types::{RawSlot, SlotReadCalls};

// Reads the open container and the player's slots into a `SlotModel`. The model building and the stack key are pure
// and tested; the game reads go through `ContainerScreenController::visual_item` and the entries of
// `SlotReadCalls`. The reader exists only when every one of those entries resolved
pub struct SlotReader {
    calls: SlotReadCalls,
    heap: MsvcHeap,
}

impl SlotReader {
    const CONTAINER_ITEMS: &'static str = "container_items";
    const HOTBAR_ITEMS: &'static str = "hotbar_items";
    const INVENTORY_ITEMS: &'static str = "inventory_items";
    // the collection of the stack the cursor carries; its one stack is at index 0. Confirmed in game 2026-11-09: a held
    // stack reads Some(Some(..)), an empty cursor Some(None) (docs/findings/vanilla-moves-ingame/report.md)
    const CURSOR_ITEMS: &'static str = "cursor_items";
    const HOTBAR_SLOTS: usize = 9;
    const PLAYER_SLOTS: usize = 36;
    // The first index of the `inventory_items` collection, which holds player slots 9..35: index = player slot - 9.
    // Confirmed in game 2026-11-09 (docs/findings/vanilla-check/report.md, Q4)
    const INVENTORY_ITEMS_FIRST_INDEX: usize = 0;
    // the first byte of an unread key's hash input; no UTF-8 id (the first bytes of a plain key's input) starts with it
    const UNREAD_MARKER: u8 = 0xFF;

    // the reader of the running build; None, with one warning that names the first missing entry, when any entry of
    // `SlotReadCalls` is missing
    pub fn bind<H: GameHost>(host: &H, heap: MsvcHeap) -> Option<Self> {
        SlotReadCalls::resolve(host)
            .inspect_err(|error| host.warn(&format!("{error}; reading the container slots is off")))
            .ok()
            .map(|calls| Self { calls, heap })
    }

    // FNV-1a, 64 bit: a fixed hash, so a key is the same in every run
    pub fn fnv1a(bytes: &[u8]) -> u64 {
        bytes
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3))
    }

    // the key of a stack: the hash of the id, one zero byte (an id has none), the aux as two little-endian bytes and
    // the user data. The fixed-size parts before the user data keep two different triples from making the same bytes
    pub fn stack_key(id: &str, aux: i16, user_data: &[u8]) -> u64 {
        let mut input = Vec::with_capacity(id.len() + 3 + user_data.len());
        input.extend_from_slice(id.as_bytes());
        input.push(0);
        input.extend_from_slice(&aux.to_le_bytes());
        input.extend_from_slice(user_data);
        Self::fnv1a(&input)
    }

    // The key of a stack whose user data could not be serialized: the hash of the marker byte, the id and the aux. It
    // holds no slot, so it stays the same when the stack moves. Two such stacks of one id and aux share it, so the
    // reader also sets `mergeable = false` and Sort never merges them. It does not use the net id, which is 0 for a
    // stack that holds a request id
    pub fn unread_key(id: &str, aux: i16) -> u64 {
        let mut input = Vec::with_capacity(id.len() + 4);
        input.push(Self::UNREAD_MARKER);
        input.extend_from_slice(id.as_bytes());
        input.push(0);
        input.extend_from_slice(&aux.to_le_bytes());
        Self::fnv1a(&input)
    }

    // the stack of one raw slot
    pub fn item(raw: &RawSlot) -> SlotItem {
        let stack_key = if raw.unread_user_data {
            Self::unread_key(&raw.id, raw.aux)
        } else {
            Self::stack_key(&raw.id, raw.aux, &raw.user_data)
        };
        SlotItem {
            id: raw.id.clone(),
            aux: raw.aux,
            count: raw.count,
            max: raw.max,
            stack_key,
            net_id: raw.net_id,
            mergeable: !raw.unread_user_data,
        }
    }

    // the player's 36 slots from the two collections: the hotbar (9 slots, player slots 0..8), then the main
    // inventory (27 slots, player slots 9..35). None for other lengths
    pub fn player_list(hotbar: Vec<Option<RawSlot>>, inventory: Vec<Option<RawSlot>>) -> Option<Vec<Option<RawSlot>>> {
        if hotbar.len() != Self::HOTBAR_SLOTS || inventory.len() != Self::PLAYER_SLOTS - Self::HOTBAR_SLOTS {
            return None;
        }
        Some(hotbar.into_iter().chain(inventory).collect())
    }

    // the model of the raw slots: the container's own slots, then the player's 36 (0..8 hotbar, 9..35 main
    // inventory). A slot with a count of 0 is empty. None when the player list is not 36 slots
    pub fn model(container: Vec<Option<RawSlot>>, player: Vec<Option<RawSlot>>) -> Option<SlotModel> {
        let items = |slots: Vec<Option<RawSlot>>| -> Vec<Option<SlotItem>> {
            slots.into_iter().map(|slot| slot.filter(|raw| raw.count > 0).map(|raw| Self::item(&raw))).collect()
        };
        let player: [Option<SlotItem>; Self::PLAYER_SLOTS] = items(player).try_into().ok()?;
        Some(SlotModel { container: items(container), player })
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

    /// the model of the open screen; None for another screen or when any slot cannot be read
    pub fn read(&self, controller: &ContainerScreenController<'_>, kind: ScreenKind) -> Option<SlotModel> {
        let slots = Self::container_slots(kind)?;
        let container = self.collection(controller, Self::CONTAINER_ITEMS, 0, slots)?;
        let hotbar = self.collection(controller, Self::HOTBAR_ITEMS, 0, Self::HOTBAR_SLOTS)?;
        let inventory = self.collection(
            controller,
            Self::INVENTORY_ITEMS,
            Self::INVENTORY_ITEMS_FIRST_INDEX,
            Self::PLAYER_SLOTS - Self::HOTBAR_SLOTS,
        )?;
        Self::model(container, Self::player_list(hotbar, inventory)?)
    }

    // the stack on the cursor: Some(None) when the cursor is empty, None when it cannot be read. Confirmed in game
    // 2026-11-09 with a held stack and an empty cursor (docs/findings/vanilla-moves-ingame/report.md)
    pub fn read_cursor(&self, controller: &ContainerScreenController<'_>) -> Option<Option<SlotItem>> {
        let mut cursor = self.collection(controller, Self::CURSOR_ITEMS, 0, 1)?;
        let raw = cursor.pop()?;
        Some(raw.filter(|raw| raw.count > 0).map(|raw| Self::item(&raw)))
    }

    // `count` slots of `collection`, from index `first`
    fn collection(
        &self,
        controller: &ContainerScreenController<'_>,
        collection: &str,
        first: usize,
        count: usize,
    ) -> Option<Vec<Option<RawSlot>>> {
        // SAFETY: the heap is the game's, as `bind` was given
        #[allow(unsafe_code)]
        let mut name = unsafe { MsvcString::new_in(&self.heap, collection) }.ok()?;
        let slots = (first..first + count)
            .map(|index| self.raw_slot(controller, &name, i32::try_from(index).ok()?))
            .collect::<Option<Vec<_>>>();
        // SAFETY: the string was made by `new_in` with this heap and is not used again
        #[allow(unsafe_code)]
        unsafe {
            name.release(&self.heap)
        };
        slots
    }

    // the raw values of one slot: Some(None) for an empty slot, None when a value cannot be read
    #[allow(unsafe_code)]
    fn raw_slot(
        &self,
        controller: &ContainerScreenController<'_>,
        name: &MsvcString,
        index: i32,
    ) -> Option<Option<RawSlot>> {
        let stack = controller.visual_item(name, index)?;
        self.raw_stack(&stack)
    }

    // the raw values of one stack, wherever it was found: Some(None) for an empty stack, None when a value cannot be
    // read
    #[allow(unsafe_code)]
    pub fn raw_stack(&self, stack: &ItemStackBaseRef<'_>) -> Option<Option<RawSlot>> {
        if stack.is_empty() {
            return Some(None);
        }
        let pointer = stack.as_ptr();
        // SAFETY: the stack is live and non-empty, and the SDK's mirror fits an ItemStackBase
        let layout = unsafe { &*pointer.cast::<ItemInstanceLayout>() };
        if layout.count == 0 {
            return Some(None);
        }
        // user data cannot be serialized yet (`SlotReadCalls::user_data_write` has no call): the stack is kept and
        // gets a key of its own
        let unread_user_data = !layout.user_data.is_null();
        Some(Some(RawSlot {
            id: stack.type_name()?,
            aux: layout.aux_value,
            count: layout.count,
            max: self.max_stack_size(stack)?,
            net_id: self.net_id(pointer)?,
            user_data: Vec::new(),
            unread_user_data,
        }))
    }

    // `ItemStackBase::getMaxStackSize`
    #[allow(unsafe_code)]
    fn max_stack_size(&self, stack: &ItemStackBaseRef<'_>) -> Option<u8> {
        if self.calls.max_stack_size == 0 {
            return None;
        }
        // SAFETY: the entry is the address of `uchar ItemStackBase::getMaxStackSize() const`: this in rcx, the size
        // in al
        let call: unsafe extern "C" fn(*const c_void) -> u8 =
            unsafe { std::mem::transmute(self.calls.max_stack_size as *const c_void) };
        // SAFETY: the stack is live and non-empty
        Some(unsafe { call(stack.as_ptr()) })
    }

    // the net id of the stack at `pointer`; 0 for a stack that holds a request id, as stratum reads it. None unless the
    // object's vtable is the `ItemStack` vtable: any other class (an `ItemInstance` is 0x80 bytes) has no net id at
    // the entry's offset, and the bytes there are never read
    #[allow(unsafe_code)]
    fn net_id(&self, pointer: *const c_void) -> Option<i32> {
        // SAFETY: the stack is live, and its first qword is its vtable
        let vtable = unsafe { RawObject::read::<usize>(pointer, 0) }?;
        if vtable != self.calls.item_stack_vtable {
            return None;
        }
        // SAFETY: the vtable is the `ItemStack` one, so the object is a whole `ItemStack`, which holds
        // `mNetIdVariant` at the entry's offset
        let variant = unsafe { RawObject::read::<ItemStackNetIdVariantLayout>(pointer, self.calls.net_id) }?;
        Some(variant.net_id().unwrap_or(0))
    }
}
