use alaydriem_hotbar_manager::game::{ContainerIds, SlotRef};
use hotbar_manager_core::structs::ScreenKind;

#[test]
fn each_container_screen_gives_the_captured_container_id() {
    let cases = [
        (ScreenKind::SmallChest, Some(7)),
        (ScreenKind::LargeChest, Some(7)),
        (ScreenKind::EnderChest, Some(7)),
        (ScreenKind::Hopper, Some(7)),
        (ScreenKind::Dispenser, Some(7)),
        (ScreenKind::Dropper, Some(7)),
        (ScreenKind::Barrel, Some(58)),
        (ScreenKind::ShulkerBox, Some(30)),
        (ScreenKind::PlayerInventory, None),
        (ScreenKind::Other, None),
    ];

    for (kind, id) in cases {
        assert_eq!(ContainerIds::for_screen(kind), id, "{kind:?}");
    }
}

#[test]
fn player_slots_use_the_hotbar_and_main_inventory_ids_with_the_player_slot_number() {
    assert_eq!((SlotRef::HOTBAR, SlotRef::INVENTORY, SlotRef::CURSOR), (28, 29, 59));
    assert_eq!(ContainerIds::player(0), (28, 0));
    assert_eq!(ContainerIds::player(8), (28, 8));
    assert_eq!(ContainerIds::player(9), (29, 9));
    assert_eq!(ContainerIds::player(35), (29, 35));
}
