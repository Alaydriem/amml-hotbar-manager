use std::cell::RefCell;
use std::ffi::c_void;

use alaydriem_hotbar_manager::game::{ContainerHandlers, ItemTransferAmount, MsvcString, ScreenActions, SlotData};
use alaydriem_hotbar_manager::types::ContainerCalls;
use hotbar_manager_core::structs::{ScreenStep, Side, SlotAddr};

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log(line: String) {
    LOG.with(|log| log.borrow_mut().push(line));
}

fn taken() -> Vec<String> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

fn text(name: *const MsvcString) -> String {
    // SAFETY: the callers pass a live string made by `MsvcString::inline`
    let name = unsafe { &*name };
    String::from_utf8_lossy(&name.buffer()[..name.size() as usize]).into_owned()
}

extern "C" fn auto_place(this: *mut c_void, amount: i32, name: *const MsvcString, index: i32) {
    log(format!("auto {} {amount:#x} {} {index}", this as usize, text(name)));
}

extern "C" fn take_all(this: *mut c_void, name: *const MsvcString, index: i32) {
    log(format!("take {} {} {index}", this as usize, text(name)));
}

extern "C" fn place_all(this: *mut c_void, name: *const MsvcString, index: i32) {
    log(format!("place {} {} {index}", this as usize, text(name)));
}

extern "C" fn drop_item(this: *mut c_void, name: *const MsvcString, index: i32, amount: *const ItemTransferAmount) {
    // SAFETY: the handlers pass a live amount
    let amount = unsafe { &*amount };
    log(format!("drop {} {} {index} {},{},{}", this as usize, text(name), amount.tag, amount.value, amount.flag));
}

extern "C" fn drop_selected(this: *mut c_void, amount: *const ItemTransferAmount) {
    // SAFETY: the handlers pass a live amount
    let amount = unsafe { &*amount };
    log(format!("cursor {} {},{},{}", this as usize, amount.tag, amount.value, amount.flag));
}

extern "C" fn swap(this: *mut c_void, a: *const SlotData, b: *const SlotData) -> u8 {
    // SAFETY: the handlers pass live slot data
    let (a, b) = unsafe { (&*a, &*b) };
    log(format!("swap {} {} {} {} {}", this as usize, text(&a.name), a.index, text(&b.name), b.index));
    1
}

const PLACE_ALL_SLOT: u64 = 54;
const SWAP_SLOT: u64 = 21;
const MANAGER_OFFSET: u64 = 0x12C0;

// a fake controller with a vtable (place all in slot 54) and a manager (swap in slot 21), laid out as the calls expect
struct World {
    controller: Vec<u64>,
    _controller_table: Vec<usize>,
    manager: Vec<u64>,
    _manager_table: Vec<usize>,
}

impl World {
    fn new() -> Self {
        let mut controller_table = vec![0usize; 64];
        controller_table[PLACE_ALL_SLOT as usize] = place_all as *const () as usize;
        let mut manager_table = vec![0usize; 40];
        manager_table[SWAP_SLOT as usize] = swap as *const () as usize;
        let mut manager = vec![0u64; 4];
        manager[0] = manager_table.as_ptr() as u64;
        let mut controller = vec![0u64; 0x1300 / 8];
        controller[0] = controller_table.as_ptr() as u64;
        controller[(MANAGER_OFFSET / 8) as usize] = manager.as_ptr() as u64;
        Self { controller, _controller_table: controller_table, manager, _manager_table: manager_table }
    }

    fn pointer(&mut self) -> *mut c_void {
        self.controller.as_mut_ptr().cast()
    }
}

fn calls() -> ContainerCalls {
    ContainerCalls {
        auto_place: auto_place as *const () as usize,
        take_all: take_all as *const () as usize,
        drop_item: drop_item as *const () as usize,
        drop_selected: drop_selected as *const () as usize,
        place_all_slot: PLACE_ALL_SLOT,
        manager: MANAGER_OFFSET,
        swap_slot: SWAP_SLOT,
    }
}

fn chest(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Container, slot }
}

fn player(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Player, slot }
}

#[test]
fn every_step_reaches_its_vanilla_handler_with_the_collection_name_and_index() {
    let mut world = World::new();
    let this = world.pointer() as usize;
    // SAFETY: a fake controller and fake handlers
    let mut handlers = unsafe { ContainerHandlers::new(world.pointer(), calls()) }.unwrap();
    taken();

    assert!(handlers.run(ScreenStep::AutoPlace(chest(4))));
    assert!(handlers.run(ScreenStep::TakeAll(player(3))));
    assert!(handlers.run(ScreenStep::PlaceAll(player(12))));
    assert!(handlers.run(ScreenStep::Swap(chest(1), player(30))));
    assert!(handlers.run(ScreenStep::DropSlot(chest(2))));
    assert!(handlers.run(ScreenStep::DropCursor));

    let manager = world.manager.as_ptr() as usize;
    assert_eq!(
        taken(),
        vec![
            format!("auto {this} 0x7fffffff container_items 4"),
            format!("take {this} hotbar_items 3"),
            format!("place {this} inventory_items 3"),
            format!("swap {manager} container_items 1 inventory_items 21"),
            format!("drop {this} container_items 2 1,0,false"),
            format!("cursor {this} 1,0,false"),
        ]
    );
}

#[test]
fn a_null_controller_or_a_zero_call_makes_no_handlers() {
    // SAFETY: the controller is null, which `new` refuses before any use
    assert!(unsafe { ContainerHandlers::new(std::ptr::null_mut(), calls()) }.is_none());
    let mut world = World::new();
    let zero = ContainerCalls { take_all: 0, ..calls() };
    // SAFETY: a fake controller
    assert!(unsafe { ContainerHandlers::new(world.pointer(), zero) }.is_none());
}

#[test]
fn a_zero_manager_offset_makes_no_handlers() {
    let mut world = World::new();
    let zero = ContainerCalls { manager: 0, ..calls() };
    // SAFETY: a fake controller
    assert!(unsafe { ContainerHandlers::new(world.pointer(), zero) }.is_none());
}

#[test]
fn a_swap_without_a_manager_cannot_be_called() {
    let mut world = World::new();
    world.controller[(MANAGER_OFFSET / 8) as usize] = 0;
    // SAFETY: a fake controller and fake handlers
    let mut handlers = unsafe { ContainerHandlers::new(world.pointer(), calls()) }.unwrap();
    taken();

    assert!(!handlers.run(ScreenStep::Swap(chest(1), chest(2))));
    assert!(taken().is_empty());
}
