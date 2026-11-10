use std::collections::HashSet;

use alaydriem_hotbar_manager::services::ButtonPainter;
use alaydriem_hotbar_manager::services::button_painter::{Quad, TextureKey};
use alaydriem_hotbar_manager::types::ButtonLook;
use hotbar_manager_core::services::ButtonLayout;
use hotbar_manager_core::structs::{ButtonAction, ButtonSpec, Glyph, PanelRect, ScreenKind};

const PANEL: PanelRect = PanelRect { x: 258.0, y: 146.0, w: 176.0, h: 166.0 };

fn spec(kind: ScreenKind, action: ButtonAction) -> ButtonSpec {
    ButtonLayout::for_screen(kind).into_iter().find(|spec| spec.action == action).unwrap()
}

// the glyph quad of a button, relative to the button's top-left
fn glyph_offset(spec: &ButtonSpec, look: ButtonLook) -> [f32; 2] {
    let rect = PANEL.resolve(spec);
    let [_, _, glyph] = ButtonPainter::plan(spec, rect, look);
    assert_eq!(glyph.quads.len(), 1);
    let quad = glyph.quads[0];
    assert_eq!(quad.size, [spec.glyph_size.0, spec.glyph_size.1]);
    assert_eq!((quad.uv, quad.uv_size), ([0.0, 0.0], [1.0, 1.0]));
    [quad.position[0] - rect.x, quad.position[1] - rect.y]
}

#[test]
fn the_glyphs_sit_where_the_easy_item_dump_pack_puts_them() {
    let dump = spec(ScreenKind::SmallChest, ButtonAction::DumpFromContainer);
    let filtered = spec(ScreenKind::Hopper, ButtonAction::DumpFromInventoryFiltered);
    let drop = spec(ScreenKind::PlayerInventory, ButtonAction::DropAll);
    let sort = spec(ScreenKind::SmallChest, ButtonAction::Sort);

    assert_eq!(glyph_offset(&dump, ButtonLook::Default), [3.5, 3.0]);
    assert_eq!(glyph_offset(&filtered, ButtonLook::Hover), [3.5, 3.0]);
    assert_eq!(glyph_offset(&drop, ButtonLook::Default), [3.0, 3.5]);
    assert_eq!(glyph_offset(&sort, ButtonLook::Default), [2.0, 2.0]);
}

#[test]
fn the_pressed_glyph_is_one_unit_lower() {
    for (kind, action) in
        [(ScreenKind::SmallChest, ButtonAction::Sort), (ScreenKind::PlayerInventory, ButtonAction::DropFiltered)]
    {
        let spec = spec(kind, action);
        let [x, y] = glyph_offset(&spec, ButtonLook::Default);
        assert_eq!(glyph_offset(&spec, ButtonLook::Pressed), [x, y + 1.0], "{action:?}");
        assert_eq!(glyph_offset(&spec, ButtonLook::Hover), [x, y], "{action:?}");
    }
}

#[test]
fn the_state_image_is_inset_one_unit_and_the_border_is_full_size() {
    let spec = spec(ScreenKind::SmallChest, ButtonAction::DumpFromInventory);
    let rect = PANEL.resolve(&spec);
    let [state, border, _] = ButtonPainter::plan(&spec, rect, ButtonLook::Default);

    let bounds = |quads: &[Quad]| {
        let left = quads.iter().map(|q| q.position[0]).fold(f32::MAX, f32::min);
        let top = quads.iter().map(|q| q.position[1]).fold(f32::MAX, f32::min);
        let right = quads.iter().map(|q| q.position[0] + q.size[0]).fold(f32::MIN, f32::max);
        let bottom = quads.iter().map(|q| q.position[1] + q.size[1]).fold(f32::MIN, f32::max);
        [left, top, right - left, bottom - top]
    };
    assert_eq!(state.texture, TextureKey::State(ButtonLook::Default));
    assert_eq!(bounds(&state.quads), [rect.x + 1.0, rect.y + 1.0, rect.w - 2.0, rect.h - 2.0]);
    assert_eq!(border.texture, TextureKey::Border);
    assert_eq!(bounds(&border.quads), [rect.x, rect.y, rect.w, rect.h]);
}

#[test]
fn each_look_draws_its_own_state_texture() {
    let spec = spec(ScreenKind::SmallChest, ButtonAction::Sort);
    for look in [ButtonLook::Default, ButtonLook::Hover, ButtonLook::Pressed] {
        let [state, _, glyph] = ButtonPainter::plan(&spec, PANEL.resolve(&spec), look);
        assert_eq!(state.texture, TextureKey::State(look));
        assert_eq!(glyph.texture, TextureKey::Glyph(Glyph::Sort));
    }
}

#[test]
fn the_colours_follow_the_light_glyph_button_and_the_pack() {
    let sort = spec(ScreenKind::SmallChest, ButtonAction::Sort);
    let dump = spec(ScreenKind::SmallChest, ButtonAction::DumpFromContainer);
    let colours =
        |spec: &ButtonSpec, look| ButtonPainter::plan(spec, PANEL.resolve(spec), look).map(|batch| batch.color);
    let white = ButtonPainter::WHITE;

    assert_eq!(
        colours(&sort, ButtonLook::Default),
        [white, ButtonPainter::BORDER_DEFAULT, ButtonPainter::GLYPH_DEFAULT]
    );
    assert_eq!(colours(&dump, ButtonLook::Default), [white, ButtonPainter::BORDER_DEFAULT, white]);
    for look in [ButtonLook::Hover, ButtonLook::Pressed] {
        assert_eq!(colours(&sort, look), [white; 3], "{look:?}");
        assert_eq!(colours(&dump, look), [white; 3], "{look:?}");
    }
    assert_eq!(ButtonPainter::GLYPH_DEFAULT, [0.3, 0.3, 0.3, 1.0]);
    assert_eq!(ButtonPainter::BORDER_DEFAULT, [0.075, 0.075, 0.075, 1.0]);
}

#[test]
fn a_nine_slice_has_one_texel_corners_and_stretched_edges() {
    let rect = PanelRect { x: 10.0, y: 20.0, w: 13.0, h: 28.0 };
    let quads = ButtonPainter::nine_slice(rect, 1.0, 4.0);

    assert_eq!(quads.len(), 9);
    assert_eq!(quads[0], Quad { position: [10.0, 20.0], size: [1.0, 1.0], uv: [0.0, 0.0], uv_size: [0.25, 0.25] });
    assert_eq!(quads[4], Quad { position: [11.0, 21.0], size: [11.0, 26.0], uv: [0.25, 0.25], uv_size: [0.5, 0.5] });
    assert_eq!(quads[8], Quad { position: [22.0, 47.0], size: [1.0, 1.0], uv: [0.75, 0.75], uv_size: [0.25, 0.25] });
    let area: f32 = quads.iter().map(|q| q.size[0] * q.size[1]).sum();
    assert_eq!(area, rect.w * rect.h);
}

#[test]
fn a_nine_slice_leaves_out_pieces_of_no_size() {
    let quads = ButtonPainter::nine_slice(PanelRect { x: 0.0, y: 0.0, w: 2.0, h: 2.0 }, 1.0, 5.0);
    assert_eq!(quads.len(), 4);
}

#[test]
fn the_look_is_pressed_before_hover() {
    let sort = ButtonAction::Sort;
    let dump = ButtonAction::DumpFromContainer;

    assert_eq!(ButtonPainter::look_of(sort, Some(sort), Some(sort)), ButtonLook::Pressed);
    assert_eq!(ButtonPainter::look_of(sort, Some(sort), None), ButtonLook::Hover);
    assert_eq!(ButtonPainter::look_of(sort, None, None), ButtonLook::Default);
    assert_eq!(ButtonPainter::look_of(sort, Some(dump), Some(dump)), ButtonLook::Default);
}

#[test]
fn every_glyph_has_its_own_file() {
    let files: HashSet<&str> = ButtonPainter::GLYPHS.iter().map(|glyph| ButtonPainter::glyph_file(*glyph)).collect();
    assert_eq!(files.len(), ButtonPainter::GLYPHS.len());
    for kind in [ScreenKind::SmallChest, ScreenKind::Hopper, ScreenKind::PlayerInventory] {
        for spec in ButtonLayout::for_screen(kind) {
            assert!(ButtonPainter::GLYPHS.contains(&spec.glyph), "{:?}", spec.glyph);
        }
    }
}

#[test]
fn the_location_hashes_match_the_spike_for_an_ascii_path() {
    // the spike's proven hash (FNV-1, unsigned bytes), the same as the game's string hash for ASCII
    let fnv1 = |bytes: &[u8]| {
        bytes
            .iter()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| hash.wrapping_mul(0x0000_0100_0000_01b3) ^ u64::from(*byte))
    };
    let path = "C:/Users/someone/AppData/Local/amml/instances/1.26.62.01-hotbar/mods/amml/@alaydriem/hotbar-manager/textures/sort";
    let (path_hash, full_hash) = ButtonPainter::location_hashes(path, 2);

    assert_eq!(path_hash, fnv1(path.as_bytes()));
    assert_eq!(full_hash, (2_u64 ^ 0xcbf2_9ce4_8422_2325).wrapping_mul(0x0000_0100_0000_01b3) ^ path_hash);
    assert_eq!(ButtonPainter::location_hashes("", 0).0, 0);
}

#[test]
fn a_queue_holds_items_when_its_begin_and_end_differ() {
    assert!(!ButtonPainter::queue_holds_items(0, 0));
    assert!(!ButtonPainter::queue_holds_items(0x1000, 0x1000));
    assert!(ButtonPainter::queue_holds_items(0x1000, 0x1030));
}
