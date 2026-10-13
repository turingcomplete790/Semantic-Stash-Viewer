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
