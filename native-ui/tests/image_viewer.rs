//! 012 T033: the image viewer in a session: opening from the grid, stepping across a page edge,
//! closing back to the grid, a failed open, the one player, and the viewer's keys.

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use player::{PlayerSnapshot, PlayerStateKind};
use semantic_stash_viewer_native::effects::{Effect, PlayerAction, Reply};
use semantic_stash_viewer_native::screens::images::{ImagesMsg, ImagesState, ViewerMsg};
use semantic_stash_viewer_native::screens::{Screen, Section};
use semantic_stash_viewer_native::session::{playback, Msg, Session};
use semantic_stash_viewer_native::shell::ShellMsg;
use stash_core::images::{ImageCard, ImagePage, ImageQuery, Media};
use stash_core::profiles::ServerProfile;

fn card(id: u32) -> ImageCard {
    ImageCard {
        id: id.to_string(),
        title: format!("Image {id}"),
        date: None,
        thumb: Some(format!("ssv-thumb://localhost/image/{id}?v=1")),
        width: 800,
        height: 600,
        media: if id == 3 {
            Media::MayAnimate
        } else {
            Media::Still
        },
    }
}

fn page_of(page: u32, size: u32, count: u32) -> ImagePage {
    let first = (page - 1) * size;
    ImagePage {
        count,
        page,
        page_size: size,
        items: (first..(first + size).min(count)).map(card).collect(),
    }
}

fn loads(effects: &[Effect]) -> Vec<(u64, u32, ImageQuery)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::LoadImagesPage {
                generation,
                page,
                query,
                ..
            } => Some((*generation, *page, query.clone())),
            _ => None,
        })
        .collect()
}

fn opens(effects: &[Effect]) -> Vec<(String, bool)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::OpenImage { id, looped, .. } => Some((id.clone(), *looped)),
            _ => None,
        })
        .collect()
}

fn profile() -> ServerProfile {
    ServerProfile {
        id: uuid::Uuid::new_v4(),
        display_name: "Testing".into(),
        base_url: "http://localhost:9998/".parse().expect("url"),
        strict_tls: false,
        api_key: None,
        created_at: chrono::Utc::now(),
        last_used_at: None,
    }
}

/// A session on Images, page 1 of 100 at the default 50 per page.
fn session() -> Session {
    let p = profile();
    let (mut s, _) = Session::enter(p.clone(), true);
    let _ = s.update(Msg::Reply(Reply::SessionLoaded {
        profile: p.id,
        saved: None,
    }));
    let effects = s
        .update(Msg::Shell(ShellMsg::Section(Section::Images)))
        .effects;
    let (generation, ..) = loads(&effects)[0].clone();
    let tab = s.shell.active().id;
    let _ = s.update(Msg::Reply(Reply::ImagesPage {
        tab,
        generation,
        result: Ok(page_of(1, 50, 100)),
    }));
    s
}

fn press(s: &mut Session, key: Key) -> Vec<Effect> {
    s.update(Msg::Key(key, Modifiers::empty())).effects
}

fn images(s: &Session) -> &ImagesState {
    match s.shell.active().current() {
        Screen::Images(i) => i,
        other => panic!("not Images: {other:?}"),
    }
}

fn snapshot(state: PlayerStateKind) -> Msg {
    Msg::Playback(playback::Msg::Snapshot(PlayerSnapshot {
        state,
        picture: Some((4000, 3000)),
        ..PlayerSnapshot::default()
    }))
}

/// Focus card `index` and press Enter.
fn open_at(s: &mut Session, index: usize) -> Vec<Effect> {
    let _ = s.update(Msg::Shell(ShellMsg::Images(ImagesMsg::List(
        semantic_stash_viewer_native::screens::browse::state::BrowseMsg::Focus(index),
    ))));
    press(s, Key::Named(Named::Enter))
}

#[test]
fn enter_opens_the_viewer_on_the_focused_card() {
    let mut s = session();
    let effects = open_at(&mut s, 2);
    assert_eq!(opens(&effects), vec![("2".to_owned(), false)]);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::PrefetchImageFiles(ids) if ids == &["3", "1"])),
        "both neighbours' files: {effects:?}"
    );
    let v = images(&s).viewer.as_ref().expect("the viewer");
    assert_eq!((v.machine.at, v.machine.count), (2, 100));
    assert_eq!(s.playback.image.as_deref(), Some("2"));
    // The player shows it: the viewer hears so and learns its size.
    let _ = s.update(snapshot(PlayerStateKind::Loading));
    let _ = s.update(snapshot(PlayerStateKind::Playing));
    let v = images(&s).viewer.as_ref().expect("the viewer");
    assert_eq!(v.picture, Some((4000, 3000)));
    assert!(s.player_visible());
    assert!(!s.now_playing(), "no now-playing bar for an image");
}

#[test]
fn an_animated_card_opens_looped() {
    let mut s = session();
    let effects = open_at(&mut s, 3);
    assert_eq!(opens(&effects), vec![("3".to_owned(), true)]);
}

#[test]
fn stepping_past_the_pages_last_card_loads_the_next_page_and_shows_its_first() {
    let mut s = session();
    open_at(&mut s, 49);
    let effects = press(&mut s, Key::Named(Named::ArrowRight));
    assert!(opens(&effects).is_empty(), "not on this page yet");
    let l = loads(&effects);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].1, 2);
    let tab = s.shell.active().id;
    let effects = s
        .update(Msg::Reply(Reply::ImagesPage {
            tab,
            generation: l[0].0,
            result: Ok(page_of(2, 50, 100)),
        }))
        .effects;
    assert_eq!(opens(&effects), vec![("50".to_owned(), false)]);
    assert_eq!(images(&s).viewer.as_ref().map(|v| v.machine.at), Some(50));
    assert_eq!(images(&s).page, 2);

    // Escape: the grid on that page with its card focused, and the image leaves the player.
    let effects = press(&mut s, Key::Named(Named::Escape));
    assert!(images(&s).viewer.is_none());
    assert_eq!((images(&s).page, images(&s).focused), (2, Some(0)));
    assert!(effects.contains(&Effect::Player(PlayerAction::Close)));
}

#[test]
fn a_neighbour_on_the_next_page_is_fetched_from_that_page() {
    let mut s = session();
    let effects = open_at(&mut s, 49);
    assert!(effects
        .iter()
        .any(|e| matches!(e, Effect::PrefetchImageFiles(ids) if ids == &["48"])));
    assert!(
        effects.iter().any(|e| matches!(
            e,
            Effect::PrefetchPageImageFiles { page: 2, size: 50, positions, .. } if positions == &[0]
        )),
        "the next page's first: {effects:?}"
    );
}

#[test]
fn back_closes_the_viewer_not_the_tab_history() {
    let mut s = session();
    open_at(&mut s, 5);
    let _ = s.update(Msg::MouseHistory(true));
    assert!(images(&s).viewer.is_none());
    assert_eq!(images(&s).focused, Some(5), "still on Images");
}

#[test]
fn a_failed_open_shows_the_message_and_the_arrows_still_move() {
    let mut s = session();
    open_at(&mut s, 4);
    let _ = s.update(Msg::Reply(Reply::ImageOpened {
        id: "4".into(),
        result: Err(stash_core::AppError::NotConnected),
    }));
    let v = images(&s).viewer.as_ref().expect("the viewer");
    assert_eq!(v.machine.failed, Some(4));
    assert!(v.error.is_some());
    let effects = press(&mut s, Key::Named(Named::ArrowRight));
    assert_eq!(opens(&effects), vec![("5".to_owned(), false)]);
    let v = images(&s).viewer.as_ref().expect("the viewer");
    assert_eq!(v.machine.failed, None);

    // The player failing to decode says so the same way.
    let _ = s.update(snapshot(PlayerStateKind::Loading));
    let _ = s.update(Msg::Playback(playback::Msg::Snapshot(PlayerSnapshot {
        state: PlayerStateKind::Error,
        error: Some(player::PlayerError::UnsupportedFormat),
        ..PlayerSnapshot::default()
    })));
    let v = images(&s).viewer.as_ref().expect("the viewer");
    assert_eq!(v.machine.failed, Some(5));
    assert_eq!(
        v.error.as_deref(),
        Some("The image's format isn't supported.")
    );
}

#[test]
fn opening_an_image_while_a_scene_plays_stops_the_scene() {
    let mut s = session();
    let _ = s.update(Msg::Shell(ShellMsg::Play("77".into())));
    assert_eq!(s.playback.playing_scene(), Some("77"));
    let effects = open_at(&mut s, 0);
    assert_eq!(opens(&effects), vec![("0".to_owned(), false)]);
    assert_eq!(
        s.playback.playing_scene(),
        None,
        "the scene's viewing ended"
    );
    assert_eq!(s.playback.image.as_deref(), Some("0"));
}

#[test]
fn playing_a_scene_closes_the_viewer() {
    let mut s = session();
    open_at(&mut s, 0);
    let _ = s.update(Msg::Shell(ShellMsg::Play("77".into())));
    assert!(images(&s).viewer.is_none());
    assert_eq!(s.playback.image, None);
}

#[test]
fn ctrl_enter_opens_the_image_in_a_new_tab() {
    let mut s = session();
    let first = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::Images(ImagesMsg::List(
        semantic_stash_viewer_native::screens::browse::state::BrowseMsg::Focus(7),
    ))));
    let effects = s
        .update(Msg::Key(Key::Named(Named::Enter), Modifiers::CTRL))
        .effects;
    assert_ne!(s.shell.active().id, first, "a new tab");
    let (generation, page, _) = loads(&effects)[0].clone();
    assert_eq!(page, 1);
    let tab = s.shell.active().id;
    let effects = s
        .update(Msg::Reply(Reply::ImagesPage {
            tab,
            generation,
            result: Ok(page_of(1, 50, 100)),
        }))
        .effects;
    assert_eq!(opens(&effects), vec![("7".to_owned(), false)]);
    assert_eq!(images(&s).viewer.as_ref().map(|v| v.machine.at), Some(7));
}

#[test]
fn the_viewers_keys_zoom_rotate_and_fit_through_the_player() {
    let mut s = session();
    open_at(&mut s, 1);
    let views = |effects: &[Effect]| -> Vec<stash_core::images::viewer::View> {
        effects
            .iter()
            .filter_map(|e| match e {
                Effect::Player(PlayerAction::View(v)) => Some(*v),
                _ => None,
            })
            .collect()
    };
    let v = views(&press(&mut s, Key::Character("+".into())));
    assert_eq!(v.len(), 1);
    assert!(v[0].zoom > 0.0);
    // Zoomed: the arrows pan instead of stepping.
    let effects = press(&mut s, Key::Named(Named::ArrowRight));
    assert!(opens(&effects).is_empty());
    assert!(views(&effects)[0].pan.0 > 0.0);
    let v = views(&press(&mut s, Key::Character("r".into())));
    assert_eq!(v[0].quarter_turns, 1);
    let shift_r = s
        .update(Msg::Key(Key::Character("R".into()), Modifiers::SHIFT))
        .effects;
    assert_eq!(views(&shift_r)[0].quarter_turns, 0);
    let v = views(&press(&mut s, Key::Character("1".into())));
    assert!(v[0].actual);
    let v = views(&press(&mut s, Key::Character("0".into())));
    assert_eq!(v[0], stash_core::images::viewer::View::default());
    // Not the player's keys: Space steps.
    let effects = press(&mut s, Key::Named(Named::Space));
    assert_eq!(opens(&effects), vec![("2".to_owned(), false)]);
}

#[test]
fn the_wheel_steps_and_ctrl_wheel_zooms_about_the_pointer() {
    let mut s = session();
    open_at(&mut s, 1);
    let wheel = |y: f32| {
        Msg::Shell(ShellMsg::Images(ImagesMsg::Viewer(ViewerMsg::Wheel {
            y,
            ctrl: false,
        })))
    };
    let effects = s.update(wheel(-1.0)).effects;
    assert_eq!(opens(&effects), vec![("2".to_owned(), false)]);
    let effects = s.update(wheel(1.0)).effects;
    assert_eq!(opens(&effects), vec![("1".to_owned(), false)]);
    let _ = s.update(Msg::Modifiers(Modifiers::CTRL));
    let _ = s.update(Msg::Shell(ShellMsg::Images(ImagesMsg::Viewer(
        ViewerMsg::Pointer(iced::Point::new(1200.0, 300.0)),
    ))));
    let effects = s.update(wheel(1.0)).effects;
    assert!(opens(&effects).is_empty());
    let view = images(&s).viewer.as_ref().expect("viewer").machine.view;
    assert!(view.zoom > 0.0);
    assert!(
        view.pan.0 < 0.0,
        "the picture moves left to keep the point right of centre still"
    );
}

#[test]
fn backspace_on_the_first_image_closes() {
    let mut s = session();
    open_at(&mut s, 1);
    let effects = press(&mut s, Key::Named(Named::Backspace));
    assert_eq!(opens(&effects), vec![("0".to_owned(), false)]);
    press(&mut s, Key::Named(Named::Backspace));
    assert!(images(&s).viewer.is_none());
}

// ---- 012 US3: the slideshow, looping images, card previews ------------------------------------

use semantic_stash_viewer_native::screens::images::viewer::SlideDue;

fn timers(effects: &[Effect]) -> Vec<(u64, SlideDue, std::time::Duration)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::SlideshowDue {
                generation,
                due,
                after,
                ..
            } => Some((*generation, *due, *after)),
            _ => None,
        })
        .collect()
}

fn due(s: &mut Session, generation: u64, due: SlideDue) -> Vec<Effect> {
    let tab = s.shell.active().id;
    s.update(Msg::Reply(Reply::SlideshowDue {
        tab,
        generation,
        due,
    }))
    .effects
}

fn playing_at(position: f64) -> Msg {
    Msg::Playback(playback::Msg::Snapshot(PlayerSnapshot {
        state: PlayerStateKind::Playing,
        position_seconds: position,
        ..PlayerSnapshot::default()
    }))
}

#[test]
fn p_starts_a_slideshow_that_moves_on_at_its_delay() {
    let mut s = session();
    open_at(&mut s, 10);
    let effects = press(&mut s, Key::Character("p".into()));
    let t = timers(&effects);
    assert_eq!(t.len(), 1);
    assert_eq!(
        (t[0].1, t[0].2),
        (SlideDue::Tick, std::time::Duration::from_secs(5))
    );
    assert!(effects
        .iter()
        .any(|e| matches!(e, Effect::PrefetchImageFiles(ids) if ids == &["11", "12"])));
    let effects = due(&mut s, t[0].0, SlideDue::Tick);
    assert_eq!(opens(&effects), vec![("11".to_owned(), false)]);
    let next = timers(&effects);
    assert_eq!(next.len(), 1, "the next image is timed");
    // An older timer does nothing.
    assert!(opens(&due(&mut s, t[0].0, SlideDue::Tick)).is_empty());

    // A step by hand pauses it: its timer does nothing, and P resumes.
    let _ = press(&mut s, Key::Named(Named::ArrowRight));
    assert!(opens(&due(&mut s, next[0].0, SlideDue::Tick)).is_empty());
    let effects = press(&mut s, Key::Character("p".into()));
    assert_eq!(timers(&effects).len(), 1);
}

#[test]
fn the_brackets_change_the_delay_and_save_it() {
    let mut s = session();
    open_at(&mut s, 0);
    let effects = press(&mut s, Key::Character("]".into()));
    assert!(effects
        .iter()
        .any(|e| matches!(e, Effect::SavePlaybackPreferences(p) if p.slideshow_delay_s == 6)));
    let effects = press(&mut s, Key::Character("p".into()));
    assert_eq!(timers(&effects)[0].2, std::time::Duration::from_secs(6));
    for _ in 0..10 {
        let _ = press(&mut s, Key::Character("[".into()));
    }
    assert_eq!(s.prefs.slideshow_delay_s, 1, "never under a second");
}

#[test]
fn a_looping_image_waits_for_its_loop_after_the_delay() {
    let mut s = session();
    open_at(&mut s, 3);
    let _ = s.update(snapshot(PlayerStateKind::Loading));
    let _ = s.update(playing_at(0.0));
    let effects = press(&mut s, Key::Character("p".into()));
    let (generation, ..) = timers(&effects)[0];
    // A loop before the delay doesn't count.
    let _ = s.update(playing_at(0.4));
    assert!(opens(&s.update(playing_at(0.0)).effects).is_empty());
    // The delay passes: it waits for the loop, at most 60 s from its start.
    let effects = due(&mut s, generation, SlideDue::Tick);
    assert!(opens(&effects).is_empty());
    let cap = timers(&effects);
    assert_eq!(
        (cap[0].1, cap[0].2),
        (SlideDue::LoopCap, std::time::Duration::from_secs(55))
    );
    let _ = s.update(playing_at(0.4));
    let effects = s.update(playing_at(0.01)).effects;
    assert_eq!(opens(&effects), vec![("4".to_owned(), false)]);
}

#[test]
fn a_looping_image_moves_on_at_the_cap_without_a_loop() {
    let mut s = session();
    open_at(&mut s, 3);
    let _ = s.update(snapshot(PlayerStateKind::Loading));
    let _ = s.update(playing_at(0.0));
    let effects = press(&mut s, Key::Character("p".into()));
    let (generation, ..) = timers(&effects)[0];
    let effects = due(&mut s, generation, SlideDue::Tick);
    let (cap, ..) = timers(&effects)[0];
    let effects = due(&mut s, cap, SlideDue::LoopCap);
    assert_eq!(opens(&effects), vec![("4".to_owned(), false)]);
}

#[test]
fn a_slideshow_in_a_tab_out_of_sight_pauses() {
    let mut s = session();
    open_at(&mut s, 0);
    let tab = s.shell.active().id;
    let effects = press(&mut s, Key::Character("p".into()));
    let (generation, ..) = timers(&effects)[0];
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let effects = s
        .update(Msg::Reply(Reply::SlideshowDue {
            tab,
            generation,
            due: SlideDue::Tick,
        }))
        .effects;
    assert!(opens(&effects).is_empty());
    let back = s.shell.tabs.iter().find(|t| t.id == tab).expect("tab");
    let Screen::Images(i) = back.current() else {
        panic!("Images")
    };
    assert!(i
        .viewer
        .as_ref()
        .and_then(|v| v.machine.slideshow)
        .is_some_and(|s| s.paused));
}

#[test]
fn an_animated_cards_hover_previews_its_own_file() {
    use semantic_stash_viewer_native::screens::browse::state::BrowseMsg;
    let mut s = session();
    let hover = |i: usize| Msg::Shell(ShellMsg::Images(ImagesMsg::List(BrowseMsg::Hover(i))));
    // A still card: nothing.
    assert!(s.update(hover(2)).effects.is_empty());
    // The animated card (3): a rest, then its preview loads as an image's.
    let effects = s.update(hover(3)).effects;
    let Some(Effect::PreviewDue { generation, .. }) = effects.first().cloned() else {
        panic!("a rest: {effects:?}")
    };
    let effects = s.update(Msg::Reply(Reply::PreviewDue(generation))).effects;
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadPreview { scene_id, .. } if scene_id == "image:3")),
        "{effects:?}"
    );
    // Opening the viewer drops it.
    let effects = open_at(&mut s, 3);
    assert!(effects.contains(&Effect::StopPreview));
}

// ---- 012 US4: details --------------------------------------------------------------------------

use semantic_stash_viewer_native::screens::images::details::{byline, facts, file_line};
use semantic_stash_viewer_native::screens::images::viewer::{panel_beside, Details};
use stash_core::images::{ImageDetails, ImageFile};
use stash_core::scenes::{GalleryChip, Named as NamedItem, PerformerChip};

fn details_loads(effects: &[Effect]) -> Vec<String> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::LoadImageDetails { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect()
}

fn full_details(id: &str) -> ImageDetails {
    ImageDetails {
        id: id.into(),
        title: format!("Image {id}"),
        code: Some("C-1".into()),
        date: Some("2024-05-01".into()),
        photographer: Some("A photographer".into()),
        details: Some("Some words.".into()),
        rating100: Some(80),
        organized: true,
        o_count: 2,
        urls: vec!["https://example.org/1".into()],
        studio: Some(NamedItem {
            id: "4".into(),
            name: "A studio".into(),
        }),
        performers: vec![PerformerChip {
            id: "2".into(),
            name: "Performer A".into(),
            image: Some("1700000000".into()),
        }],
        tags: vec![NamedItem {
            id: "6".into(),
            name: "A tag".into(),
        }],
        galleries: vec![GalleryChip {
            id: "9".into(),
            title: "A gallery".into(),
            image_count: 12,
        }],
        file: Some(ImageFile {
            path: "/library/a.jpg".into(),
            size: 3_200_000,
            format: "jpeg".into(),
            width: 4000,
            height: 3000,
            duration: None,
        }),
    }
}

#[test]
fn i_loads_the_details_once_per_image_and_the_panel_follows_the_arrows() {
    let mut s = session();
    open_at(&mut s, 2);
    let effects = press(&mut s, Key::Character("i".into()));
    assert_eq!(details_loads(&effects), vec!["2"]);
    // Closed and opened again on the same image: nothing new.
    let _ = press(&mut s, Key::Character("i".into()));
    assert!(details_loads(&press(&mut s, Key::Character("i".into()))).is_empty());
    let tab = s.shell.active().id;
    let effects = s
        .update(Msg::Reply(Reply::ImageDetails {
            tab,
            id: "2".into(),
            result: Box::new(Ok(full_details("2"))),
        }))
        .effects;
    let v = images(&s).viewer.as_ref().expect("viewer");
    assert!(matches!(
        v.details.as_ref().map(|d| &d.state),
        Some(Details::Ready(_))
    ));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadThumbnails { keys, .. } if keys.len() == 1)),
        "the performer's picture: {effects:?}"
    );
    // The panel follows the next image.
    let effects = press(&mut s, Key::Named(Named::ArrowRight));
    assert_eq!(details_loads(&effects), vec!["3"]);
    // A late answer for the image before doesn't land.
    let _ = s.update(Msg::Reply(Reply::ImageDetails {
        tab,
        id: "2".into(),
        result: Box::new(Ok(full_details("2"))),
    }));
    let v = images(&s).viewer.as_ref().expect("viewer");
    assert_eq!(v.details.as_ref().map(|d| d.id.as_str()), Some("3"));
    assert!(matches!(
        v.details.as_ref().map(|d| &d.state),
        Some(Details::Loading)
    ));
}

#[test]
fn without_the_panel_no_details_are_read() {
    let mut s = session();
    let effects = open_at(&mut s, 2);
    assert!(details_loads(&effects).is_empty());
    assert!(details_loads(&press(&mut s, Key::Named(Named::ArrowRight))).is_empty());
}

#[test]
fn every_field_shows_and_empty_ones_are_left_out() {
    let d = full_details("1");
    assert_eq!(
        byline(&d).as_deref(),
        Some("C-1 · 2024-05-01 · by A photographer")
    );
    assert_eq!(facts(&d).as_deref(), Some("★★★★☆ · Organized · O-count 2"));
    assert_eq!(
        file_line(d.file.as_ref().expect("file")),
        "JPEG · 4000×3000 · 3.2 MB"
    );
    let empty = ImageDetails {
        code: None,
        date: None,
        photographer: None,
        rating100: None,
        organized: false,
        o_count: 0,
        ..full_details("1")
    };
    assert_eq!(byline(&empty), None);
    assert_eq!(facts(&empty), None);
}

#[test]
fn a_tag_chip_filters_images_here_or_in_a_new_tab() {
    use stash_core::scenes::filter::kinds::CriterionKind;
    use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier};
    let tag = Criterion::new(
        CriterionKind::Tags,
        Modifier::IncludesAll,
        CriterionValue::Items {
            items: vec![Item {
                id: "6".into(),
                label: "A tag".into(),
            }],
            excluded: vec![],
            depth: Some(0),
        },
    );
    let mut s = session();
    open_at(&mut s, 2);
    let first = s.shell.active().id;
    // Ctrl: a new tab, the viewer left as it was.
    let _ = s.update(Msg::Modifiers(Modifiers::CTRL));
    let effects = s
        .update(Msg::Shell(ShellMsg::FilterImagesTo {
            criterion: tag.clone(),
            new_tab: false,
        }))
        .effects;
    assert_ne!(s.shell.active().id, first);
    assert_eq!(images(&s).query.criteria, vec![tag.clone()]);
    assert_eq!(loads(&effects)[0].1, 1);
    let _ = s.update(Msg::Modifiers(Modifiers::empty()));
    let _ = s.update(Msg::Shell(ShellMsg::Select(first)));
    // Here: the image leaves the player and Images opens filtered.
    let _ = press(&mut s, Key::Named(Named::ArrowRight));
    let effects = s
        .update(Msg::Shell(ShellMsg::FilterImagesTo {
            criterion: tag.clone(),
            new_tab: false,
        }))
        .effects;
    assert!(effects.contains(&Effect::Player(PlayerAction::Close)));
    assert_eq!(images(&s).query.criteria, vec![tag]);
    assert!(images(&s).viewer.is_none());
    assert!(!s.player_visible());
    assert!(!s.now_playing(), "no bar while the image closes");
    // Back returns (even before the player has said it closed) to the viewer on the image it was on, shown again.
    let effects = s.update(Msg::Shell(ShellMsg::Back)).effects;
    assert!(images(&s).query.criteria.is_empty());
    assert_eq!(images(&s).viewer.as_ref().map(|v| v.machine.at), Some(3));
    assert_eq!(opens(&effects), vec![("3".to_owned(), true)]);
    assert_eq!(s.playback.image.as_deref(), Some("3"));
    // Back again (Escape's twin) closes the viewer to its grid.
    let _ = s.update(Msg::Shell(ShellMsg::Back));
    assert!(images(&s).viewer.is_none());
}

#[test]
fn a_tab_switch_back_to_an_open_viewer_shows_its_image_again() {
    let mut s = session();
    open_at(&mut s, 5);
    let first = s.shell.active().id;
    let _ = s.update(Msg::Shell(ShellMsg::NewTab));
    let _ = s.update(Msg::Shell(ShellMsg::Play("77".into())));
    let effects = s.update(Msg::Shell(ShellMsg::Select(first))).effects;
    // A scene took the player, so the viewer was closed: nothing to show again.
    assert!(opens(&effects).is_empty());
    assert!(images(&s).viewer.is_none());
}

#[test]
fn under_900_px_the_panel_overlays_the_picture() {
    use semantic_stash_viewer_native::screens::scenes::layout::Layout;
    let wide = Layout::new(1600.0, 1000.0);
    let narrow = Layout::new(800.0, 1000.0);
    assert!(panel_beside(&wide));
    assert!(!panel_beside(&narrow));
    let mut v = semantic_stash_viewer_native::screens::images::ViewerState::default();
    v.machine.details = true;
    assert!(v.area(&wide).0 < 1600.0, "beside: the picture is narrower");
    assert_eq!(
        v.area(&narrow).0,
        800.0,
        "over: the picture keeps the width"
    );
}

/// Back and Forward walk viewer → chip's grid → viewer → grid and back the same way, the image
/// shown again each time the viewer returns.
#[test]
fn forward_undoes_back_through_the_viewer_and_a_chips_grid() {
    use stash_core::scenes::filter::kinds::CriterionKind;
    use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier};
    let tag = Criterion::new(
        CriterionKind::Tags,
        Modifier::IncludesAll,
        CriterionValue::Items {
            items: vec![Item {
                id: "6".into(),
                label: "A tag".into(),
            }],
            excluded: vec![],
            depth: Some(0),
        },
    );
    let mut s = session();
    open_at(&mut s, 2);
    let _ = press(&mut s, Key::Named(Named::ArrowRight));
    let _ = s.update(Msg::Shell(ShellMsg::FilterImagesTo {
        criterion: tag.clone(),
        new_tab: false,
    }));
    let at = |s: &Session| images(s).viewer.as_ref().map(|v| v.machine.at);
    let filtered = |s: &Session| !images(s).query.criteria.is_empty();

    // Back: the viewer on image 3; Back again: its grid, 3 focused.
    let effects = s.update(Msg::Shell(ShellMsg::Back)).effects;
    assert_eq!((filtered(&s), at(&s)), (false, Some(3)));
    assert_eq!(opens(&effects), vec![("3".to_owned(), true)]);
    let effects = s.update(Msg::Shell(ShellMsg::Back)).effects;
    assert_eq!((filtered(&s), at(&s)), (false, None));
    assert_eq!(images(&s).focused, Some(3));
    assert!(effects.contains(&Effect::Player(PlayerAction::Close)));
    assert!(s.shell.can_go_forward());

    // Forward: the viewer again on 3; Forward again: the chip's grid, the image let go.
    let effects = s.update(Msg::Shell(ShellMsg::Forward)).effects;
    assert_eq!((filtered(&s), at(&s)), (false, Some(3)));
    assert_eq!(opens(&effects), vec![("3".to_owned(), true)]);
    let effects = s.update(Msg::Shell(ShellMsg::Forward)).effects;
    assert!(filtered(&s));
    assert!(effects.contains(&Effect::Player(PlayerAction::Close)));
    assert!(!s.shell.can_go_forward());

    // And back once more to the viewer.
    let effects = s.update(Msg::Shell(ShellMsg::Back)).effects;
    assert_eq!((filtered(&s), at(&s)), (false, Some(3)));
    assert_eq!(opens(&effects), vec![("3".to_owned(), true)]);
}

#[test]
fn opening_a_new_viewer_drops_the_old_way_forward() {
    let mut s = session();
    open_at(&mut s, 2);
    let _ = press(&mut s, Key::Named(Named::Escape));
    assert!(s.shell.can_go_forward(), "Forward reopens the viewer");
    open_at(&mut s, 5);
    let _ = press(&mut s, Key::Named(Named::Escape));
    let effects = s.update(Msg::Shell(ShellMsg::Forward)).effects;
    assert_eq!(
        opens(&effects),
        vec![("5".to_owned(), false)],
        "the newer viewer"
    );
}
