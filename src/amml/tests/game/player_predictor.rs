use alaydriem_hotbar_manager::game::PlayerPredictor;
use hotbar_manager_core::services::SlotModel;
use hotbar_manager_core::structs::{ItemMove, Side, SlotAddr, SlotItem};

fn item(net_id: i32, count: u8) -> Option<SlotItem> {
    Some(SlotItem { id: "minecraft:stone".into(), aux: 0, count, max: 64, stack_key: 1, net_id, mergeable: true })
}

fn player(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Player, slot }
}

fn chest(slot: u8) -> SlotAddr {
    SlotAddr { side: Side::Container, slot }
}

fn model() -> SlotModel {
    let mut model = SlotModel { container: vec![None; 27], player: std::array::from_fn(|_| None) };
    model.player[0] = item(1, 10);
    model.player[9] = item(2, 20);
    model.container[0] = item(3, 30);
    model
}

#[test]
fn a_swap_of_two_player_slots_is_predicted_with_the_games_slot_swap() {
    let mv = ItemMove::Swap { a: player(0), b: player(9) };

    assert_eq!(PlayerPredictor::swap_pair(&mv, &model()), Some((0, 9)));
}

#[test]
fn a_whole_stack_placed_into_an_empty_player_slot_is_predicted_as_a_swap() {
    let mv = ItemMove::Place { from: player(9), to: player(20), count: 20 };

    assert_eq!(PlayerPredictor::swap_pair(&mv, &model()), Some((9, 20)));
}

#[test]
fn moves_the_slot_swap_cannot_show_are_not_predicted() {
    let cases = [
        // part of a stack
        ItemMove::Place { from: player(9), to: player(20), count: 5 },
        // onto a stack
        ItemMove::Place { from: player(9), to: player(0), count: 20 },
        // the container side has no safe setter
        ItemMove::Swap { a: chest(0), b: player(0) },
        ItemMove::Place { from: chest(0), to: player(20), count: 30 },
        ItemMove::Drop { from: player(0), count: 10 },
    ];

    for mv in cases {
        assert_eq!(PlayerPredictor::swap_pair(&mv, &model()), None, "{mv:?}");
    }
}
