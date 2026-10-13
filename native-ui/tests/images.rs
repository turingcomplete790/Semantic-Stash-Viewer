//! 012 US1 (T022): finding images. Opening Images, requerying, saved image filters, paging with
//! prefetch, criteria scoped to images, the card size, and the keys (FR-001–FR-007).

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use semantic_stash_viewer_native::effects::{Effect, Reply};
use semantic_stash_viewer_native::screens::browse::filters::{menu_rows, MenuRow};
use semantic_stash_viewer_native::screens::browse::state::{BrowseMsg, Panel, SEARCH_ID};
use semantic_stash_viewer_native::screens::images::{ImagesMsg, ImagesState, CARD_SIZES};
use semantic_stash_viewer_native::screens::scenes::layout::Layout;
use semantic_stash_viewer_native::screens::{Mode, Screen, Section};
use semantic_stash_viewer_native::session::{Msg, Session};
use semantic_stash_viewer_native::shell::{Shell, ShellMsg};
use stash_core::browse::query::SortDirection;
use stash_core::images::{ImageCard, ImagePage, ImageQuery, ImageSort, Media};
use stash_core::profiles::ServerProfile;
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::saved::{DisplayMode, SavedFilterOf};
use stash_core::scenes::filter::{Criterion, CriterionValue, Modifier};

// `Shell` for the shell-level tests, `Session` for keys.

const TAB: u64 = 1;

fn layout() -> Layout {
    Layout::new(1100.0, 900.0)
}

fn loads(effects: &[Effect]) -> Vec<(u64, u32, u32, ImageQuery)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::LoadImagesPage {
                generation,
                page,
                size,
                query,
                ..
            } => Some((*generation, *page, *size, query.clone())),
            _ => None,
        })
        .collect()
}

fn card(id: u32) -> ImageCard {
    ImageCard {
        id: id.to_string(),
        title: format!("Image {id}"),
        date: None,
        thumb: Some(format!("ssv-thumb://localhost/image/{id}?v=1")),
        width: 800,
        height: 600,
        media: Media::Still,
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

/// Images showing `page` of 300 at 40 per page.
fn ready(page: u32) -> ImagesState {
    let mut s = ImagesState::default();
    s.list.page = page;
    s.list.page_size = 40;
    let effects = s.enter(TAB);
    let (generation, ..) = loads(&effects)[0].clone();
    let _ = s.page_loaded(generation, Ok(page_of(page, 40, 300)), false);
    s
}

fn update(s: &mut ImagesState, m: BrowseMsg<ImageSort>) -> Vec<Effect> {
    s.update(ImagesMsg::List(m), TAB, &layout()).effects
}

fn organized() -> Criterion {
    Criterion::new(
        CriterionKind::Organized,
        Modifier::Equals,
        CriterionValue::Bool(true),
    )
}

// ---- Opening Images ----------------------------------------------------------------------------

#[test]
fn opening_images_requests_page_one_in_the_web_uis_default_order() {
    let (mut shell, _) = Shell::new();
    let step = shell.update(ShellMsg::Section(Section::Images));
    let l = loads(&step.effects);
    assert_eq!(l.len(), 1, "one request: {:?}", step.effects);
    assert_eq!(l[0].1, 1);
    assert_eq!(l[0].3.sort, ImageSort::Path);
    assert_eq!(l[0].3.direction, SortDirection::Asc);
    assert_eq!(shell.active().current().title(), "Images");
    assert_eq!(shell.active().current().section(), Some(Section::Images));
}

#[test]
fn opening_images_uses_the_servers_default_image_filter() {
    let (mut shell, _) = Shell::new();
    shell.default_images = Some(SavedFilterOf {
        id: "default".into(),
        name: "Default".into(),
        query: ImageQuery {
            sort: ImageSort::Date,
            direction: SortDirection::Desc,
            criteria: vec![organized()],
            ..ImageQuery::default()
        },
        page_size: Some(60),
        mode: Some(DisplayMode::Wall),
        left_out: vec![],
    });
    let step = shell.update(ShellMsg::Section(Section::Images));
    let l = loads(&step.effects);
    assert_eq!(l.len(), 1);
    let (_, page, size, query) = &l[0];
    assert_eq!((*page, *size), (1, 60));
    assert_eq!(query.sort, ImageSort::Date);
    assert_eq!(query.criteria, vec![organized()]);
    let Screen::Images(s) = shell.active().current() else {
        panic!("Images")
    };
    assert_eq!(s.mode, Mode::Wall);
    assert_eq!(
        s.applied, None,
        "the default isn't shown as an applied filter"
    );
}

#[test]
fn a_page_reply_lands_on_the_images_screen_that_asked() {
    let (mut shell, _) = Shell::new();
    let step = shell.update(ShellMsg::Section(Section::Images));
    let (generation, ..) = loads(&step.effects)[0].clone();
    let effects = shell
        .images_page(TAB, generation, Ok(page_of(1, 40, 300)), false)
        .effects;
    let Screen::Images(s) = shell.active().current() else {
        panic!("Images")
    };
    assert_eq!(s.cards().len(), 40);
    assert_eq!(s.count(), Some(300));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::PrefetchImages { pages, .. } if pages == &vec![2])),
        "the next page is prefetched: {effects:?}"
    );
    assert_eq!(shell.active_thumbs().len(), 40);
    assert!(shell.active_thumbs()[0].0.starts_with("image:"));
}

// ---- Requery from page 1 -----------------------------------------------------------------------

#[test]
fn search_a_criterion_the_sort_and_the_direction_requery_from_page_one() {
    let mut s = ready(3);
    update(&mut s, BrowseMsg::SearchInput("beach".into()));
    let l = loads(&update(&mut s, BrowseMsg::SearchSubmit));
    assert_eq!((l[0].1, l[0].3.search.as_str()), (1, "beach"));

    let mut s = ready(3);
    update(&mut s, BrowseMsg::FilterTo(organized()));
    assert_eq!(s.page, 1);
    assert_eq!(s.query.criteria, vec![organized()]);

    let mut s = ready(3);
    let l = loads(&update(&mut s, BrowseMsg::Sort(ImageSort::Date)));
    assert_eq!((l[0].1, l[0].3.sort), (1, ImageSort::Date));

    let mut s = ready(3);
    let l = loads(&update(&mut s, BrowseMsg::ToggleDirection));
    assert_eq!((l[0].1, l[0].3.direction), (1, SortDirection::Desc));
}

#[test]
fn random_gets_a_seed_and_reshuffle_a_new_one() {
    let mut s = ready(2);
    let l = loads(&update(&mut s, BrowseMsg::Sort(ImageSort::Random)));
    let seed = l[0].3.seed.expect("a seed");
    let l = loads(&update(&mut s, BrowseMsg::Reshuffle));
    assert_ne!(l[0].3.seed, Some(seed));
    assert_eq!(l[0].1, 1);
}

// ---- Saved filters -----------------------------------------------------------------------------

#[test]
fn a_saved_image_filter_applies_its_query_page_size_and_mode() {
    let mut s = ready(4);
    let filter = SavedFilterOf {
        id: "4".into(),
        name: "SSV images performer A".into(),
        query: ImageQuery {
            search: "shore".into(),
            sort: ImageSort::Rating,
            criteria: vec![organized()],
            ..ImageQuery::default()
        },
        // Not one the web UI offers: the nearest is used.
        page_size: Some(100),
        mode: Some(DisplayMode::Wall),
        left_out: vec![],
    };
    let l = loads(&update(&mut s, BrowseMsg::ApplySaved(Box::new(filter))));
    assert_eq!(l.len(), 1);
    let (_, page, size, query) = &l[0];
    assert_eq!((*page, *size), (1, 120));
    assert_eq!(query.sort, ImageSort::Rating);
    assert_eq!(query.search, "shore");
    assert_eq!(s.mode, Mode::Wall);
    assert_eq!(
        s.applied.as_ref().map(|a| a.name.as_str()),
        Some("SSV images performer A")
    );
}

#[test]
fn a_saved_filters_list_mode_leaves_images_in_their_mode() {
    let mut s = ready(1);
    let filter = SavedFilterOf {
        id: "9".into(),
        name: "Listed".into(),
        query: ImageQuery::images_default(),
        page_size: None,
        mode: Some(DisplayMode::List),
        left_out: vec![],
    };
    update(&mut s, BrowseMsg::ApplySaved(Box::new(filter)));
    assert_eq!(s.mode, Mode::Grid, "images have no list mode");
}

#[test]
fn the_saved_filters_menu_asks_for_the_image_filters() {
    let mut s = ready(1);
    let step = s.update(ImagesMsg::List(BrowseMsg::SavedMenu(true)), TAB, &layout());
    assert_eq!(
        step.up,
        Some(
            semantic_stash_viewer_native::screens::images::ImagesUp::Need(
                semantic_stash_viewer_native::library::Need::SavedImageFilters
            )
        )
    );
    assert!(matches!(s.panel, Panel::Saved { .. }));
}

// ---- Paging ------------------------------------------------------------------------------------

#[test]
fn paging_requests_one_page_and_prefetches_its_neighbours() {
    let mut s = ready(3);
    let effects = update(&mut s, BrowseMsg::Page(4));
    let l = loads(&effects);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].1, 4);
    let effects = s
        .page_loaded(l[0].0, Ok(page_of(4, 40, 300)), false)
        .effects;
    let prefetch: Vec<_> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::PrefetchImages { pages, warm, .. } => Some((pages.clone(), *warm)),
            _ => None,
        })
        .collect();
    assert_eq!(prefetch, vec![(vec![5, 3], Some(5))]);
    assert!(loads(&effects).is_empty(), "no other page is requested");
}

#[test]
fn an_older_reply_is_ignored() {
    let mut s = ready(1);
    let first = loads(&update(&mut s, BrowseMsg::Page(2)))[0].0;
    let second = loads(&update(&mut s, BrowseMsg::Page(3)))[0].0;
    assert!(s
        .page_loaded(first, Ok(page_of(2, 40, 300)), false)
        .effects
        .is_empty());
    let _ = s.page_loaded(second, Ok(page_of(3, 40, 300)), false);
    assert_eq!(s.cards()[0].id, "80");
}

// ---- Criteria scoped to images -----------------------------------------------------------------

#[test]
fn scene_only_criteria_are_not_offered() {
    let mut s = ready(1);
    let kinds: Vec<CriterionKind> = menu_rows(&s.list)
        .into_iter()
        .filter_map(|r| match r {
            MenuRow::Kind(k) => Some(k),
            _ => None,
        })
        .collect();
    assert!(kinds.contains(&CriterionKind::Photographer));
    assert!(kinds.contains(&CriterionKind::Folder));
    assert!(kinds.contains(&CriterionKind::Galleries));
    for scene_only in [
        CriterionKind::Duration,
        CriterionKind::Interactive,
        CriterionKind::Groups,
    ] {
        assert!(!kinds.contains(&scene_only), "{scene_only:?} offered");
        let step = s.update(ImagesMsg::List(BrowseMsg::Add(scene_only)), TAB, &layout());
        assert!(step.up.is_none() && step.effects.is_empty());
        assert_eq!(s.panel, Panel::None);
    }
}

#[test]
fn the_folder_criterion_picks_from_the_folders_list() {
    use stash_core::adapter::choices::ChoiceKind;
    let mut s = ready(1);
    let step = s.update(
        ImagesMsg::List(BrowseMsg::Add(CriterionKind::Folder)),
        TAB,
        &layout(),
    );
    assert_eq!(
        step.up,
        Some(
            semantic_stash_viewer_native::screens::images::ImagesUp::Need(
                semantic_stash_viewer_native::library::Need::Choices(ChoiceKind::Folders)
            )
        )
    );
}

// ---- Card size ---------------------------------------------------------------------------------

#[test]
fn the_card_size_changes_the_columns_and_keeps_the_first_visible_card() {
    let mut s = ready(1);
    let base = layout();
    let cols = |s: &ImagesState| s.layout(&base).cols(Mode::Grid);
    let medium = cols(&s);
    s.scroll = s.layout(&base).row_top(12, Mode::Grid);
    let first = s.layout(&base).first_visible(s.scroll, Mode::Grid);
    let step = s.update(ImagesMsg::CardSize(0), TAB, &base);
    assert!(cols(&s) > medium, "smaller cards, more columns");
    assert!(matches!(step.effects[..], [Effect::ScrollTo { .. }]));
    let now = s.layout(&base).first_visible(s.scroll, Mode::Grid);
    assert!(
        now <= first && first < now + cols(&s),
        "card {first} still in the first row"
    );
    let _ = s.update(ImagesMsg::CardSize(9), TAB, &base);
    assert_eq!(s.card_size as usize, CARD_SIZES.len() - 1, "clamped");
    assert!(cols(&s) < medium);
    assert!(loads(&step.effects).is_empty(), "no request");
}

#[test]
fn image_pictures_are_square() {
    let s = ImagesState::default();
    let l = s.layout(&layout());
    assert_eq!(l.thumb_height(), l.card_width().floor());
    assert_eq!(l.wall_height(), l.wall_width().floor());
}

// ---- Keys --------------------------------------------------------------------------------------

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

fn session() -> Session {
    let p = profile();
    let (mut s, _) = Session::enter(p.clone(), true);
    let _ = s.update(Msg::Reply(Reply::SessionLoaded {
        profile: p.id,
        saved: None,
    }));
    s
}

fn session_on_images() -> Session {
    let mut session = session();
    let effects = session
        .update(Msg::Shell(ShellMsg::Section(Section::Images)))
        .effects;
    let (generation, ..) = loads(&effects)[0].clone();
    let effects = session
        .update(Msg::Reply(Reply::ImagesPage {
            tab: session.shell.active().id,
            generation,
            result: Ok(page_of(1, 40, 300)),
        }))
        .effects;
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::LoadThumbnails { keys, .. } if keys.len() == 40)),
        "the page's thumbnails are asked for: {effects:?}"
    );
    session
}

#[test]
fn the_servers_default_image_filter_comes_from_the_ui_settings() {
    let mut s = session();
    let ui = stash_core::adapter::filters::parse_ui_settings(
        &serde_json::from_str(
            r#"{"defaultFilters":{"images":{
        "find_filter":{"sort":"date","direction":"DESC","per_page":40},
        "object_filter":{"organized":{"value":"true","modifier":"EQUALS"}}}}}"#,
        )
        .expect("json"),
    );
    let _ = s.update(Msg::Reply(Reply::UiSettings(Ok(ui))));
    let _ = s.update(Msg::Shell(ShellMsg::Section(Section::Images)));
    let state = images(&s);
    assert_eq!(state.query.sort, ImageSort::Date);
    assert_eq!(state.query.direction, SortDirection::Desc);
    assert_eq!(state.query.criteria, vec![organized()]);
}

fn press(session: &mut Session, key: Key) -> Vec<Effect> {
    session.update(Msg::Key(key, Modifiers::empty())).effects
}

fn images(session: &Session) -> &ImagesState {
    match session.shell.active().current() {
        Screen::Images(s) => s,
        _ => panic!("Images"),
    }
}

#[test]
fn the_grid_keys_move_between_images() {
    let mut s = session_on_images();
    press(&mut s, Key::Named(Named::ArrowRight));
    assert_eq!(images(&s).focused, Some(0));
    press(&mut s, Key::Named(Named::ArrowRight));
    assert_eq!(images(&s).focused, Some(1));
    press(&mut s, Key::Named(Named::End));
    assert_eq!(images(&s).focused, Some(39));
    let effects = press(&mut s, Key::Character("]".into()));
    assert_eq!(loads(&effects)[0].1, 2);
}

#[test]
fn slash_f_and_s_search_filter_and_list_saved_filters() {
    let mut s = session_on_images();
    let effects = press(&mut s, Key::Character("/".into()));
    assert!(effects
        .iter()
        .any(|e| matches!(e, Effect::Focus(id) if *id == SEARCH_ID)));
    press(&mut s, Key::Character("f".into()));
    assert!(matches!(images(&s).panel, Panel::Menu { .. }));
    press(&mut s, Key::Named(Named::Escape));
    assert_eq!(images(&s).panel, Panel::None);
    let effects = press(&mut s, Key::Character("s".into()));
    assert!(matches!(images(&s).panel, Panel::Saved { .. }));
    assert!(effects
        .iter()
        .any(|e| matches!(e, Effect::LoadSavedImageFilters)));
}
