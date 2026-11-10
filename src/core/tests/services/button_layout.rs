use hotbar_manager_core::services::ButtonLayout;
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, Glyph, PanelRect, ScreenKind};

fn spec(
    action: ButtonAction,
    glyph: Glyph,
    offset: (f32, f32),
    size: (f32, f32),
    glyph_size: (f32, f32),
) -> ButtonSpec {
    ButtonSpec { action, offset, size, glyph, glyph_size }
}

fn side(action: ButtonAction, glyph: Glyph, offset: (f32, f32)) -> ButtonSpec {
    spec(action, glyph, offset, (15.0, 30.0), (8.0, 24.0))
}

fn sort() -> ButtonSpec {
    spec(ButtonAction::Sort, Glyph::Sort, (142.0, 7.0), (11.0, 11.0), (7.0, 7.0))
}

fn chest_specs() -> Vec<ButtonSpec> {
    vec![
        side(ButtonAction::DumpFromInventory, Glyph::DumpFromInventory, (-30.0, 0.0)),
        side(ButtonAction::DumpFromContainer, Glyph::DumpFromContainer, (-15.0, 0.0)),
        side(ButtonAction::DumpFromInventoryFiltered, Glyph::DumpFromInventoryFiltered, (-30.0, 30.0)),
        side(ButtonAction::DumpFromContainerFiltered, Glyph::DumpFromContainerFiltered, (-15.0, 30.0)),
        sort(),
    ]
}

fn small_container_specs() -> Vec<ButtonSpec> {
    vec![
        side(ButtonAction::DumpFromContainerFiltered, Glyph::DumpFromContainerFiltered, (-15.0, 7.0)),
        side(ButtonAction::DumpFromInventoryFiltered, Glyph::DumpFromInventoryFiltered, (-30.0, 7.0)),
        sort(),
    ]
}

#[test]
fn small_chest_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::SmallChest), chest_specs());
}

#[test]
fn large_chest_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::LargeChest), chest_specs());
}

#[test]
fn barrel_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::Barrel), chest_specs());
}

#[test]
fn shulker_box_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::ShulkerBox), chest_specs());
}

#[test]
fn ender_chest_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::EnderChest), chest_specs());
}

#[test]
fn hopper_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::Hopper), small_container_specs());
}

#[test]
fn dispenser_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::Dispenser), small_container_specs());
}

#[test]
fn dropper_layout() {
    assert_eq!(ButtonLayout::for_screen(ScreenKind::Dropper), small_container_specs());
}

#[test]
fn player_inventory_layout_has_no_sort() {
    let expected = vec![
        spec(ButtonAction::DropAll, Glyph::Drop, (0.0, -30.0), (30.0, 15.0), (24.0, 8.0)),
        spec(ButtonAction::DropFiltered, Glyph::DropFiltered, (0.0, -15.0), (30.0, 15.0), (24.0, 8.0)),
    ];
    assert_eq!(ButtonLayout::for_screen(ScreenKind::PlayerInventory), expected);
}

#[test]
fn other_screens_have_no_buttons() {
    assert!(ButtonLayout::for_screen(ScreenKind::Other).is_empty());
}

#[test]
fn resolve_places_the_button_from_the_panel_top_left() {
    let panel = PanelRect { x: 258.0, y: 146.0, w: 176.0, h: 166.0 };
    assert_eq!(panel.resolve(&sort()), PanelRect { x: 400.0, y: 153.0, w: 11.0, h: 11.0 });
    let drop_all = spec(ButtonAction::DropAll, Glyph::Drop, (0.0, -30.0), (30.0, 15.0), (24.0, 8.0));
    assert_eq!(panel.resolve(&drop_all), PanelRect { x: 258.0, y: 116.0, w: 30.0, h: 15.0 });
}

#[test]
fn only_containers_are_sortable() {
    let sortable = [
        ScreenKind::SmallChest,
        ScreenKind::LargeChest,
        ScreenKind::Barrel,
        ScreenKind::ShulkerBox,
        ScreenKind::EnderChest,
        ScreenKind::Hopper,
        ScreenKind::Dispenser,
        ScreenKind::Dropper,
    ];
    for kind in sortable {
        assert!(kind.sortable(), "{kind:?}");
    }
    assert!(!ScreenKind::PlayerInventory.sortable());
    assert!(!ScreenKind::Other.sortable());
}
