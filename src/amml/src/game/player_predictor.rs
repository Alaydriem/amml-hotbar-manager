use std::collections::VecDeque;
use std::ffi::c_void;

use amml_game::RawObject;
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ItemMove, Side, SlotAddr};

use crate::game::PlayerInventory;
use crate::traits::MovePredictor;
use crate::types::PlayerCalls;

// The client prediction of the move engine, the way stratum keeps the client inventory in step with its own
// requests: a move between two player slots that the game's `FillingContainer::swapSlots` can show (a swap, or a
// whole stack into an empty slot) is shown at once, and swapped back when the server refuses it or no answer comes,
// only while the slot still holds the moved stack. Every other move waits for the server's answer: no safe setter for
// the open container's slots was found (see the task 9b report). Holds the predictions of the requests in flight, in
// the order they were made (one for a single move, several for a batch)
pub struct PlayerPredictor {
    calls: Option<PlayerCalls>,
    client: *mut c_void,
    // the predictions in flight, oldest first: the two slots, and the net id of the stack moved into the second
    done: VecDeque<(u8, u8, i32)>,
}

// SAFETY: the client pointer is a game object used on the game thread only, under the runtime's lock; another thread
// only moves the value or drops it at disable
#[allow(unsafe_code)]
unsafe impl Send for PlayerPredictor {}

impl Default for PlayerPredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerPredictor {
    pub fn new() -> Self {
        Self { calls: None, client: std::ptr::null_mut(), done: VecDeque::new() }
    }

    // enable: the calls of the running build; without them nothing is predicted
    pub fn bind(&mut self, calls: Option<PlayerCalls>) {
        self.calls = calls;
        self.done.clear();
    }

    // whether the calls are bound, so that moves can be predicted
    pub fn bound(&self) -> bool {
        self.calls.is_some()
    }

    /// the client of this tick, whose local player's inventory is predicted
    ///
    /// # Safety
    /// `client` must be null or the live `ClientInstance`, and stay live until the next call
    #[allow(unsafe_code)]
    pub unsafe fn set_client(&mut self, client: *mut c_void) {
        self.client = client;
    }

    // the two player slots the game's slot swap exchanges to show `mv` on `before`; None for a move it cannot show
    pub fn swap_pair(mv: &ItemMove, before: &SlotModel) -> Option<(u8, u8)> {
        let filled = |addr: SlotAddr| before.player.get(usize::from(addr.slot)).and_then(Option::as_ref);
        match *mv {
            ItemMove::Swap { a, b } if Self::both_player(a, b) => Some((a.slot, b.slot)),
            ItemMove::Place { from, to, count } if Self::both_player(from, to) => {
                let whole = filled(from).is_some_and(|source| source.count == count);
                let empty = before.player.get(usize::from(to.slot)).is_some_and(Option::is_none);
                (whole && empty).then_some((from.slot, to.slot))
            }
            _ => None,
        }
    }

    fn both_player(a: SlotAddr, b: SlotAddr) -> bool {
        a.side == Side::Player && b.side == Side::Player && a != b
    }

    // the local player's inventory of the client of this tick
    fn inventory(&self) -> Option<PlayerInventory> {
        let calls = self.calls.as_ref()?;
        if self.client.is_null() {
            return None;
        }
        // SAFETY: the client of this tick, live until the next tick, whose vtable holds getLocalPlayer at the
        // entry's slot
        #[allow(unsafe_code)]
        let function = unsafe { RawObject::slot(self.client, calls.local_player_slot) }?;
        // SAFETY: the slot holds `LocalPlayer* getLocalPlayer() const`
        #[allow(unsafe_code)]
        let player = unsafe { RawObject::call_pointer(function, self.client) }?;
        // SAFETY: the client's live local player
        #[allow(unsafe_code)]
        unsafe {
            PlayerInventory::of_player(player, calls)
        }
    }
}

impl MovePredictor for PlayerPredictor {
    fn predict(&mut self, mv: &ItemMove, before: &SlotModel) -> bool {
        let Some((a, b)) = Self::swap_pair(mv, before) else { return false };
        let Some(inventory) = self.inventory() else { return false };
        let Some(moved) = inventory.net_id(usize::from(a)) else { return false };
        inventory.swap_slots(i32::from(a), i32::from(b));
        self.done.push_back((a, b, moved));
        true
    }

    fn confirm(&mut self, _mv: &ItemMove) {
        self.done.pop_front();
    }

    fn undo(&mut self, _mv: &ItemMove) -> bool {
        let Some((a, b, moved)) = self.done.pop_back() else { return false };
        let Some(inventory) = self.inventory() else { return false };
        if inventory.net_id(usize::from(b)) != Some(moved) {
            return false;
        }
        inventory.swap_slots(i32::from(a), i32::from(b));
        true
    }
}
