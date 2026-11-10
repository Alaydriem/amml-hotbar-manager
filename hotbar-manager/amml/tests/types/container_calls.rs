use alaydriem_hotbar_manager::types::ContainerCalls;

use crate::support::fake_host::FakeHost;

fn host_without(missing: &[&str]) -> FakeHost {
    let entries: Vec<(&'static str, u64)> =
        ContainerCalls::ENTRIES.into_iter().zip(0x2000..).filter(|(entry, _)| !missing.contains(entry)).collect();
    FakeHost::new(&entries)
}

#[test]
fn container_calls_resolve_and_name_the_first_missing_entry() {
    let calls = ContainerCalls::resolve(&host_without(&[])).unwrap();
    assert_eq!(calls.auto_place, 0x2000);
    assert_eq!(calls.take_all, 0x2001);
    assert_eq!(calls.place_all_slot, 0x2002);
    assert_eq!(calls.drop_item, 0x2003);
    assert_eq!(calls.drop_selected, 0x2004);
    assert_eq!(calls.manager, 0x2005);
    assert_eq!(calls.swap_slot, 0x2006);

    for entry in ContainerCalls::ENTRIES {
        let host = host_without(&[entry]);
        let error = ContainerCalls::resolve(&host).unwrap_err();
        assert!(error.to_string().contains(entry), "{entry}: {error}");
        assert_eq!(ContainerCalls::missing(&host), Some(entry));
    }
}
