use crossterm::event::KeyCode;
use muxget::controllers::app::App;
use muxget::controllers::keys::Dialog;
use muxget::models::download::{Download, Status};
use muxget::models::queue::{Queue, DEFAULT};

fn app_with(statuses: &[Status]) -> App {
    // Explicit queues and a throwaway config dir: no dependence on, and no
    // damage to, whatever the user has saved.
    std::env::set_var("XDG_CONFIG_HOME", std::env::temp_dir().join("muxget-tests"));
    let mut app = App::with_queues(".".into(), vec![Queue::new(DEFAULT, "default", 3)]);
    app.downloads = statuses
        .iter()
        .enumerate()
        .map(|(id, s)| Download {
            id,
            queue: muxget::models::queue::DEFAULT,
            url: format!("https://example.com/{id}.iso"),
            backend: "aria2c",
            status: s.clone(),
            progress: Default::default(),
            over: Default::default(),
        path: None,
        pid: None,
        tries: 0,
        })
        .collect();
    app
}


fn typed(app: &mut App, keys: &str) {
    for c in keys.chars() {
        app.on_key(KeyCode::Char(c));
    }
}

fn url_field(app: &App) -> String {
    match &app.dialog {
        Some(Dialog::Add(form)) => form.fields[0].clone(),
        other => panic!("expected the add form, got {other:?}"),
    }
}

#[test]
fn add_dialog_captures_typing_until_escape() {
    let mut app = app_with(&[]);

    app.on_key(KeyCode::Char('a'));
    typed(&mut app, "htp");
    app.on_key(KeyCode::Backspace);
    assert_eq!(url_field(&app), "ht");

    // While a dialog is open, `q` types instead of quitting.
    assert!(!app.on_key(KeyCode::Char('q')));
    assert_eq!(url_field(&app), "htq");

    app.on_key(KeyCode::Esc);
    assert_eq!(app.dialog, None);
    assert!(app.downloads.is_empty());
    assert!(app.on_key(KeyCode::Char('q')), "quits once the dialog is closed");
}

#[test]
fn edit_dialog_prefills_the_selected_url() {
    let mut app = app_with(&[Status::Running, Status::Done]);
    app.selected = 1;

    app.on_key(KeyCode::Char('e'));
    assert_eq!(
        app.dialog,
        Some(Dialog::Edit(1, "https://example.com/1.iso".into()))
    );

    app.on_key(KeyCode::Esc);
    assert_eq!(app.downloads.len(), 2, "escaping changes nothing");
}

#[test]
fn delete_dialog_confirms_before_removing() {
    let mut app = app_with(&[Status::Running, Status::Done, Status::Running]);
    app.selected = 1;

    app.on_key(KeyCode::Char('d'));
    assert_eq!(app.dialog, Some(Dialog::Delete(1)));

    app.on_key(KeyCode::Char('n'));
    assert_eq!(app.downloads.len(), 3, "declining keeps the row");

    app.on_key(KeyCode::Char('d'));
    app.on_key(KeyCode::Char('y'));
    assert_eq!(app.downloads.len(), 2);
    assert_eq!(app.downloads[1].id, 2, "the right row was removed");
    assert!(app.visible().contains(&app.selected), "selection stays valid");
}

#[test]
fn key_sequences_show_a_menu_and_swallow_the_next_key() {
    let mut app = app_with(&[]);

    app.on_key(KeyCode::Char('g'));
    assert_eq!(app.pending, Some('g'), "menu is up, waiting for the second key");

    // The second key is consumed by the sequence, never by the normal keymap.
    app.on_key(KeyCode::Char('n'));
    assert_eq!(app.pending, None);
    assert_eq!(app.dialog, Some(Dialog::QueueNew(String::new())));
    app.on_key(KeyCode::Esc);

    // An unbound second key cancels quietly.
    typed(&mut app, "gz");
    assert_eq!(app.pending, None);
    assert_eq!(app.dialog, None);

    // Sequence navigation.
    app.add_queue("media");
    typed(&mut app, "gk");
    assert_eq!(app.queue().name, "default");
    typed(&mut app, "gj");
    assert_eq!(app.queue().name, "media");

    typed(&mut app, "g+");
    assert_eq!(app.queue().max_active, 4);
    typed(&mut app, "g-");
    assert_eq!(app.queue().max_active, 3);
}

#[test]
fn zz_quits_but_a_lone_z_does_not() {
    let mut app = app_with(&[]);
    assert!(!app.on_key(KeyCode::Char('Z')));
    assert!(app.on_key(KeyCode::Char('Z')));

    let mut app = app_with(&[]);
    app.on_key(KeyCode::Char('Z'));
    assert!(!app.on_key(KeyCode::Char('x')), "Zx is not a quit");
}

#[test]
fn the_add_form_carries_per_item_settings_into_the_download() {
    use crossterm::event::KeyCode::Tab;

    let mut app = app_with(&[]);
    app.on_key(KeyCode::Char('a'));
    typed(&mut app, "https://example.com/a.iso");
    app.on_key(Tab); // range — left empty
    app.on_key(Tab);
    typed(&mut app, "/tmp/here");
    app.on_key(Tab);
    typed(&mut app, "mine.iso");
    app.on_key(Tab);
    typed(&mut app, "2M");
    app.on_key(KeyCode::Enter);

    assert_eq!(app.dialog, None);
    let d = &app.downloads[0];
    assert_eq!(d.url, "https://example.com/a.iso");
    assert_eq!(d.over.dir, "/tmp/here");
    assert_eq!(d.over.name, "mine.iso");
    assert_eq!(d.over.rate, "2M");

    // A plain add leaves every override empty.
    app.add("https://example.com/b.iso");
    assert!(app.downloads[1].over.is_empty());
}

#[test]
fn a_pattern_and_range_add_one_download_per_number() {
    use crossterm::event::KeyCode::Tab;

    let mut app = app_with(&[]);
    app.on_key(KeyCode::Char('a'));
    typed(&mut app, "https://example.com/f%02d.iso");
    app.on_key(Tab);
    typed(&mut app, "1-3");
    app.on_key(Tab);
    app.on_key(Tab);
    typed(&mut app, "local%d.iso");
    app.on_key(KeyCode::Enter);

    let urls: Vec<&str> = app.downloads.iter().map(|d| d.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://example.com/f01.iso",
            "https://example.com/f02.iso",
            "https://example.com/f03.iso"
        ]
    );
    assert_eq!(app.downloads[2].over.name, "local3.iso");
}

#[test]
fn the_mouse_selects_rows_queues_and_filters() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    use muxget::controllers::downloads::Filter;
    use muxget::views::ui::layout;
    use ratatui::layout::{Rect, Size};

    // Wide and tall enough for the sidebar and the graph to be drawn.
    let size = Size::new(120, 40);
    let panes = layout(Rect::new(0, 0, size.width, size.height));
    let click = |x: u16, y: u16| MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: crossterm::event::KeyModifiers::NONE,
    };

    let mut app = app_with(&[Status::Queued, Status::Queued, Status::Queued]);
    app.add_queue("media");
    app.current = 0;

    // Third row of the list: one border, one header, then the rows.
    let list = panes.list;
    app.on_mouse(click(list.x + 4, list.y + 4), size);
    assert_eq!(app.selected, 2);

    // The header row selects nothing and must not panic.
    app.on_mouse(click(list.x + 4, list.y + 1), size);
    assert_eq!(app.selected, 2);

    // Second queue in the sidebar.
    let queues = panes.queues.expect("sidebar drawn at this width");
    app.on_mouse(click(queues.x + 2, queues.y + 2), size);
    assert_eq!(app.current, 1);

    // Second filter in the sidebar.
    let filters = panes.filters.expect("sidebar drawn at this width");
    app.on_mouse(click(filters.x + 2, filters.y + 2), size);
    assert_eq!(app.filter, Filter::ALL[1]);

    // A dialog owns the keyboard and the mouse with it.
    app.current = 0;
    app.on_key(KeyCode::Char('a'));
    app.on_mouse(click(queues.x + 2, queues.y + 2), size);
    assert_eq!(app.current, 0, "clicks are ignored while a dialog is open");
}

#[test]
fn rows_and_queues_can_be_reordered_from_the_keyboard() {
    let mut app = app_with(&[Status::Queued, Status::Queued]);
    let (first, second) = (app.downloads[0].url.clone(), app.downloads[1].url.clone());
    // Order is priority, so J promotes the row below.
    app.selected = 0;
    app.on_key(KeyCode::Char('J'));
    assert_eq!(app.downloads[0].url, second);
    assert_eq!(app.downloads[1].url, first);
    app.on_key(KeyCode::Char('K'));
    assert_eq!(app.downloads[0].url, first, "and back");

    app.add_queue("media");
    assert_eq!(app.current, 1);
    typed(&mut app, "g<");
    assert_eq!(app.current, 0, "the queue moved left, and the view followed it");
    assert_eq!(app.queues[0].name, "media");
}

#[test]
fn selection_marks_rows_and_operations_act_on_them() {
    let mut app = app_with(&[Status::Running, Status::Running, Status::Running, Status::Running]);

    // Space marks the row under the cursor.
    app.on_key(KeyCode::Char(' '));
    assert_eq!(app.marked, [0], "marks are download ids, not row numbers");

    // `M` extends from the last mark to the cursor.
    app.on_key(KeyCode::Down);
    app.on_key(KeyCode::Down);
    app.on_key(KeyCode::Char('M'));
    assert_eq!(app.marked, [0, 1, 2], "the whole span, inclusive");

    // An operation acts on every marked row, then the marks are dropped.
    app.on_key(KeyCode::Char('p'));
    let paused = app.downloads.iter().filter(|d| d.status == Status::Paused).count();
    assert_eq!(paused, 3);
    assert!(app.marked.is_empty(), "a finished operation clears the selection");

    // With nothing marked, an operation is about the cursor row alone.
    app.selected = 3;
    app.on_key(KeyCode::Char('x'));
    assert_eq!(app.downloads[3].status, Status::Cancelled);
    assert_eq!(app.downloads[0].status, Status::Paused, "the others are untouched");

    // `A` marks everything on screen, and again clears it.
    app.on_key(KeyCode::Char('A'));
    assert_eq!(app.marked.len(), 4);
    app.on_key(KeyCode::Char('A'));
    assert!(app.marked.is_empty());

    // Deleting a selection removes exactly those rows, whatever their index.
    app.selected = 1;
    app.on_key(KeyCode::Char(' '));
    app.selected = 3;
    app.on_key(KeyCode::Char(' '));
    app.on_key(KeyCode::Char('d'));
    assert!(matches!(app.dialog, Some(Dialog::Delete(_))), "still asks first");
    app.on_key(KeyCode::Char('y'));
    let ids: Vec<usize> = app.downloads.iter().map(|d| d.id).collect();
    assert_eq!(ids, [0, 2], "the marked ids are gone, the rest kept");
    assert!(app.selected < app.downloads.len(), "the cursor stays in bounds");
}

#[test]
fn vim_movements_walk_the_list() {
    use crossterm::event::{KeyEvent, KeyModifiers};

    let rows: Vec<Status> = (0..9).map(|_| Status::Running).collect();
    let mut app = app_with(&rows);
    app.page = 4;

    // A count repeats the movement.
    typed(&mut app, "3j");
    assert_eq!(app.selected, 3);
    typed(&mut app, "2k");
    assert_eq!(app.selected, 1);
    // Counts are for the command right after them and no further.
    typed(&mut app, "j");
    assert_eq!(app.selected, 2, "the earlier count is spent");

    // `G` is the bottom, `NG` the nth row, `gg` the top.
    app.on_key(KeyCode::Char('G'));
    assert_eq!(app.selected, 8);
    typed(&mut app, "4G");
    assert_eq!(app.selected, 3);
    typed(&mut app, "gg");
    assert_eq!(app.selected, 0, "a count survives the `g` of a sequence");
    typed(&mut app, "7gg");
    assert_eq!(app.selected, 6);

    // Paging is measured in rows the list can show.
    let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
    app.on_key_event(ctrl('u'));
    assert_eq!(app.selected, 4, "half a page up");
    app.on_key_event(ctrl('f'));
    assert_eq!(app.selected, 8, "a whole page down, clamped at the end");
    app.on_key_event(ctrl('b'));
    assert_eq!(app.selected, 4);

    // Control chords are not the plain keys: `d` alone still asks to delete.
    app.on_key(KeyCode::Char('d'));
    assert!(app.dialog.is_some());
}

/// The editing keys have to survive the whole route from the terminal event:
/// `on_key_event` is what carries the modifiers, and a plain `on_key` loses
/// them by design.
#[test]
fn form_fields_take_the_usual_editing_keys() {
    use crossterm::event::{KeyEvent, KeyModifiers};

    let mut app = app_with(&[]);
    let press = |app: &mut App, code, mods| {
        app.on_key_event(KeyEvent::new(code, mods));
    };

    app.on_key(KeyCode::Char('a'));
    typed(&mut app, "https://y.com/watch?v=abc");

    // Alt-Backspace takes one part of the url.
    press(&mut app, KeyCode::Backspace, KeyModifiers::ALT);
    assert_eq!(url_field(&app), "https://y.com/watch?v=");

    // The caret moves, and typing happens where it is rather than at the end.
    press(&mut app, KeyCode::Home, KeyModifiers::NONE);
    press(&mut app, KeyCode::Delete, KeyModifiers::NONE);
    assert_eq!(url_field(&app), "ttps://y.com/watch?v=");
    typed(&mut app, "h");
    assert_eq!(url_field(&app), "https://y.com/watch?v=", "typed at the caret");

    // Moving to the next field puts the caret at its end, not the last one's.
    press(&mut app, KeyCode::Tab, KeyModifiers::NONE);
    typed(&mut app, "1-3");
    press(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
    match &app.dialog {
        Some(Dialog::Add(form)) => assert_eq!(form.fields[1], "1-"),
        other => panic!("expected the add form, got {other:?}"),
    }
}
