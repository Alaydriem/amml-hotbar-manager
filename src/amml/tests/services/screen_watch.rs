use alaydriem_hotbar_manager::services::ScreenWatch;

#[test]
fn no_view_drew_means_no_screen() {
    assert_eq!(ScreenWatch::new().open_view(1.0), None);
}

#[test]
fn a_view_that_drew_lately_is_the_open_screen() {
    let mut watch = ScreenWatch::new();

    watch.seen(0x10, 1.0);

    assert_eq!(watch.open_view(1.0 + ScreenWatch::OPEN_SECONDS), Some(0x10));
}

#[test]
fn a_view_that_stopped_drawing_is_a_closed_screen() {
    let mut watch = ScreenWatch::new();
    watch.seen(0x10, 1.0);

    assert_eq!(watch.open_view(1.0 + ScreenWatch::OPEN_SECONDS + 0.01), None);
}

#[test]
fn the_newest_view_wins() {
    let mut watch = ScreenWatch::new();
    watch.seen(0x10, 1.0);
    watch.seen(0x20, 1.1);

    assert_eq!(watch.open_view(1.2), Some(0x20));
}

#[test]
fn clear_closes_the_screen() {
    let mut watch = ScreenWatch::new();
    watch.seen(0x10, 1.0);

    watch.clear();

    assert_eq!(watch.open_view(1.0), None);
}
