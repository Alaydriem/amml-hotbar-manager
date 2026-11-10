use std::collections::BTreeMap;

use hotbar_manager_core::services::SortPlanner;
use hotbar_manager_core::structs::SlotItem;

fn stack(id: &str, aux: i16, count: u8, max: u8, key: u64, net_id: i32) -> SlotItem {
    SlotItem { id: format!("minecraft:{id}"), aux, count, max, stack_key: key, net_id, mergeable: true }
}

fn pad(mut items: Vec<Option<SlotItem>>, size: usize) -> Vec<Option<SlotItem>> {
    items.resize(size, None);
    items
}

/// The item totals per (id, aux, key): a target never adds, loses or changes an item.
fn totals(v: &[Option<SlotItem>]) -> BTreeMap<(String, i16, u64), u32> {
    let mut totals = BTreeMap::new();
    for item in v.iter().flatten() {
        *totals.entry((item.id.clone(), item.aux, item.stack_key)).or_insert(0) += u32::from(item.count);
    }
    totals
}

/// The target, checked: same size, same totals, no stack over its max.
fn run(container: &[Option<SlotItem>]) -> Vec<Option<SlotItem>> {
    let target = SortPlanner::target(container);
    assert_eq!(target.len(), container.len());
    assert_eq!(totals(&target), totals(container));
    assert!(target.iter().flatten().all(|item| item.count <= item.max));
    target
}

fn counts(v: &[Option<SlotItem>]) -> Vec<Option<(String, u8)>> {
    v.iter().map(|s| s.as_ref().map(|i| (i.id.replace("minecraft:", ""), i.count))).collect()
}

fn some(id: &str, count: u8) -> Option<(String, u8)> {
    Some((id.to_string(), count))
}

#[test]
fn sorts_by_id_then_count_desc() {
    let c = pad(
        vec![
            Some(stack("dirt", 0, 10, 64, 1, 1)),
            Some(stack("cobblestone", 0, 5, 64, 2, 2)),
            Some(stack("cobblestone", 0, 64, 64, 2, 3)),
        ],
        9,
    );
    let target = run(&c);
    // the 5 and 64 stacks have the same key: merge gives 64 + 5 = 69 -> 64, 5
    assert_eq!(
        counts(&target),
        vec![some("cobblestone", 64), some("cobblestone", 5), some("dirt", 10), None, None, None, None, None, None]
    );
}

#[test]
fn merges_part_stacks_first() {
    let c = pad((0..3).map(|i| Some(stack("cobblestone", 0, 20, 64, 7, i))).collect(), 9);
    let target = run(&c);
    let mut want = vec![some("cobblestone", 60)];
    want.resize(9, None);
    assert_eq!(counts(&target), want);
}

#[test]
fn never_merges_different_stack_keys() {
    let c = pad(
        vec![
            Some(stack("diamond_sword", 0, 1, 1, 1, 1)),
            Some(stack("diamond_sword", 0, 1, 1, 1, 2)),
            Some(stack("enchanted_book", 0, 1, 1, 10, 3)),
            Some(stack("enchanted_book", 0, 1, 1, 11, 4)),
        ],
        9,
    );
    let target = run(&c);
    assert_eq!(target.iter().flatten().count(), 4);
    assert!(target.iter().flatten().all(|i| i.count == 1));

    // part-stacks with room, same id, different keys: both stay as they are
    let c = pad(vec![Some(stack("potion", 0, 3, 16, 10, 1)), Some(stack("potion", 0, 5, 16, 11, 2))], 9);
    let target = run(&c);
    assert_eq!(counts(&target)[..3], [some("potion", 5), some("potion", 3), None]);
    assert_eq!(target[0].as_ref().map(|i| i.stack_key), Some(11));
    assert_eq!(target[1].as_ref().map(|i| i.stack_key), Some(10));
}

#[test]
fn duplicate_net_ids_still_sort() {
    let c = pad(
        vec![
            Some(stack("c", 0, 1, 64, 1, 0)),
            Some(stack("a", 0, 1, 64, 2, 0)),
            Some(stack("b", 0, 1, 64, 3, 0)),
            Some(stack("a", 0, 1, 64, 4, 0)),
        ],
        9,
    );
    let target = run(&c);
    let keys: Vec<u64> = target.iter().flatten().map(|i| i.stack_key).collect();
    assert_eq!(keys, vec![2, 4, 3, 1]);
}

#[test]
fn an_over_max_stack_keeps_the_layout() {
    let c = pad(vec![Some(stack("b", 0, 20, 16, 1, 1)), Some(stack("a", 0, 1, 64, 2, 2))], 9);
    assert_eq!(SortPlanner::target(&c), c);
}

#[test]
fn an_oversized_container_keeps_the_layout() {
    let c: Vec<Option<SlotItem>> =
        (0..257).map(|i| Some(stack(&format!("i{:03}", 300 - i), 0, 1, 64, i as u64, i))).collect();
    assert_eq!(SortPlanner::target(&c), c);
}

#[test]
fn respects_small_max() {
    let c = pad(vec![Some(stack("ender_pearl", 0, 10, 16, 5, 1)), Some(stack("ender_pearl", 0, 10, 16, 5, 2))], 9);
    let target = run(&c);
    assert_eq!(counts(&target)[..3], [some("ender_pearl", 16), some("ender_pearl", 4), None]);
}

#[test]
fn ties_aux_then_original_order() {
    let c = pad(
        vec![
            Some(stack("wool", 3, 64, 64, 3, 1)),
            Some(stack("wool", 1, 64, 64, 1, 2)),
            Some(stack("banner", 0, 1, 16, 20, 3)),
            Some(stack("banner", 0, 1, 16, 21, 4)),
        ],
        9,
    );
    let target = run(&c);
    let ids: Vec<(String, i16, i32)> = target.iter().flatten().map(|i| (i.id.clone(), i.aux, i.net_id)).collect();
    assert_eq!(
        ids,
        vec![
            ("minecraft:banner".to_string(), 0, 3),
            ("minecraft:banner".to_string(), 0, 4),
            ("minecraft:wool".to_string(), 1, 2),
            ("minecraft:wool".to_string(), 3, 1),
        ]
    );
}

#[test]
fn empty_slots_go_last() {
    let c = vec![None, Some(stack("b", 0, 1, 64, 1, 1)), None, Some(stack("a", 0, 1, 64, 2, 2)), None];
    let target = run(&c);
    assert_eq!(counts(&target), vec![some("a", 1), some("b", 1), None, None, None]);
}

#[test]
fn an_already_sorted_container_is_its_own_target() {
    let c = pad(vec![Some(stack("a", 0, 9, 64, 1, 1)), Some(stack("b", 0, 3, 64, 2, 2))], 9);
    assert_eq!(SortPlanner::target(&c), c);
}

#[test]
fn works_for_any_size() {
    for size in [5usize, 9, 27, 54] {
        let c: Vec<Option<SlotItem>> = (0..size)
            .map(|i| {
                (i % 4 != 1).then(|| {
                    let n = (size - i) % 7;
                    stack(&format!("item_{n}"), 0, 1 + (i % 50) as u8, 64, n as u64, i as i32)
                })
            })
            .collect();
        let target = run(&c);
        assert!(target.windows(2).all(|pair| pair[0].is_some() || pair[1].is_none()), "empty slots last");
    }
}

#[test]
fn unmergeable_stacks_are_never_merged() {
    let plain = SlotItem { mergeable: false, ..stack("stone", 0, 1, 64, 7, 1) };
    let c = pad(vec![Some(plain.clone()), Some(plain)], 27);

    let target = run(&c);

    assert_eq!(target, c);
}
