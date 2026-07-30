use muxget::controllers::app::{App, Filter};
use muxget::models::download::{Download, Status};

fn app_with(statuses: &[Status]) -> App {
    let mut app = App::new(".".into());
    app.downloads = statuses
        .iter()
        .map(|s| Download {
            url: "https://example.com/a.iso".into(),
            backend: "aria2c",
            status: s.clone(),
            progress: Default::default(),
        })
        .collect();
    app
}

#[test]
fn filter_selects_rows_and_keeps_selection_visible() {
    let mut app = app_with(&[Status::Running, Status::Done, Status::Running]);

    assert_eq!(app.visible(), [0, 1, 2]);

    app.set_filter(Filter::Done);
    assert_eq!(app.visible(), [1]);
    assert_eq!(app.selected, 1, "selection jumps to a shown row");

    app.set_filter(Filter::Active);
    assert_eq!(app.visible(), [0, 2]);
    assert_eq!(app.selected, 0);
}

#[test]
fn selection_moves_within_the_filtered_rows_only() {
    let mut app = app_with(&[Status::Running, Status::Done, Status::Running]);
    app.set_filter(Filter::Active);

    app.move_selection(1);
    assert_eq!(app.selected, 2, "skips the filtered-out row");
    app.move_selection(1);
    assert_eq!(app.selected, 2, "clamps at the end");
    app.move_selection(-5);
    assert_eq!(app.selected, 0, "clamps at the start");
}

#[test]
fn failed_filter_covers_cancelled() {
    let app = app_with(&[Status::Cancelled, Status::Failed("exit 1".into()), Status::Done]);
    assert_eq!(app.visible().len(), 3);
    assert!(Filter::Failed.matches(&Status::Cancelled));
    assert!(!Filter::Failed.matches(&Status::Done));
}
