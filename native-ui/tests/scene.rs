//! 007 T047: the Scene screen's transitions (spec US4; capability C15).

use semantic_stash_viewer_native::effects::Effect;
use semantic_stash_viewer_native::screens::scene::{Details, SceneState};
use semantic_stash_viewer_native::screens::Screen;
use semantic_stash_viewer_native::shell::{Shell, ShellMsg, Up};
use stash_core::scenes::SceneDetails;
use stash_core::AppError;

const TAB: u64 = 1;

fn details(id: &str) -> SceneDetails {
    SceneDetails {
        id: id.into(),
        title: "Seven".into(),
        code: None,
        date: Some("2026-01-02".into()),
        details: Some("A description.".into()),
        director: None,
        studio: Some(stash_core::scenes::Named {
            id: "4".into(),
            name: "A studio".into(),
        }),
        performers: vec![stash_core::scenes::PerformerChip {
            id: "2".into(),
            name: "Someone".into(),
            image: Some("1791406802".into()),
        }],
        tags: vec![stash_core::scenes::Named {
            id: "6".into(),
            name: "A tag".into(),
        }],
        groups: vec![],
        markers: vec![stash_core::scenes::Marker {
            id: "1".into(),
            title: "Start".into(),
            seconds: 42.5,
            end_seconds: None,
            tag: "A tag".into(),
        }],
        urls: vec![],
        organized: false,
        resume_time: None,
        play_duration: None,
        file_count: 2,
        sprite: None,
        vtt: None,
        rating100: Some(80),
        play_count: 3,
        o_count: 0,
        duration_seconds: Some(125.0),
        file_name: Some("seven.mp4".into()),
        file: None,
        cover_version: "1".into(),
    }
}

#[test]
fn entering_loads_the_details_and_cover_once() {
    let mut s = SceneState::new("7", "Seven");
    let effects = s.enter(TAB);
    assert!(effects.contains(&Effect::LoadSceneDetails {
        tab: TAB,
        id: "7".into()
    }));
    assert!(effects.contains(&Effect::LoadCover {
        tab: TAB,
        id: "7".into()
    }));
    assert!(s.enter(TAB).is_empty(), "already on their way");
    s.details_loaded(Ok(details("7")));
    assert!(matches!(s.details, Details::Ready(_)));
    assert!(s.enter(TAB).is_empty(), "here already");
}

#[test]
fn a_restored_tab_shows_its_title_before_the_details_arrive() {
    let json = r#"{"Scene":{"scene_id":"7","title":"Seven"}}"#;
    let screen: Screen = serde_json::from_str(json).expect("parse");
    assert_eq!(screen.title(), "Seven");
    let Screen::Scene(s) = screen else { panic!() };
    assert!(matches!(s.details, Details::Loading));
}

#[test]
fn the_title_follows_the_details() {
    let mut s = SceneState::new("7", "an old title");
    let _ = s.enter(TAB);
    s.details_loaded(Ok(details("7")));
    assert_eq!(s.title, "Seven");
}

#[test]
fn a_details_failure_shows_its_message() {
    let mut s = SceneState::new("7", "Seven");
    let _ = s.enter(TAB);
    s.details_loaded(Err(AppError::SceneNotFound { id: "7".into() }));
    let Details::Failed(message) = &s.details else {
        panic!("expected a failure")
    };
    assert!(!message.is_empty());
    // Entering again retries.
    assert!(s
        .enter(TAB)
        .iter()
        .any(|e| matches!(e, Effect::LoadSceneDetails { .. })));
}

#[test]
fn play_bubbles_up_with_the_scene() {
    let (mut shell, _) = Shell::new();
    let _ = shell.update(ShellMsg::Open(Screen::scene("7", "Seven")));
    let step = shell.update(ShellMsg::Play("7".into()));
    assert_eq!(step.up, Some(Up::Play("7".into())));
}

#[test]
fn results_land_on_the_screen_that_asked() {
    let (mut shell, _) = Shell::new();
    let tab = shell.active().id;
    let _ = shell.update(ShellMsg::Open(Screen::scene("7", "Seven")));
    shell.scene_details(tab, "7", Ok(details("7")));
    let Screen::Scene(s) = shell.active().current() else {
        panic!()
    };
    assert!(matches!(s.details, Details::Ready(_)));
    // Another scene's result changes nothing.
    shell.scene_details(tab, "8", Err(AppError::SceneNotFound { id: "8".into() }));
    let Screen::Scene(s) = shell.active().current() else {
        panic!()
    };
    assert!(matches!(s.details, Details::Ready(_)));
}

// ---- 008 US2: the full scene page ----------------------------------------------------------

use semantic_stash_viewer_native::screens::scene::{ChipRef, SceneMsg, SceneUp, Section};
use stash_core::adapter::chips::ChipKind;

fn ready_state() -> SceneState {
    let mut s = SceneState::new("7", "Seven");
    let _ = s.enter(TAB);
    s.details_loaded(Ok(details("7")));
    s
}

#[test]
fn a_section_loads_once_then_opens_and_closes() {
    let mut s = ready_state();
    let effects = s.update(SceneMsg::ToggleFiles, TAB).effects;
    assert_eq!(
        effects,
        vec![Effect::LoadSceneFiles {
            tab: TAB,
            id: "7".into()
        }]
    );
    assert_eq!(s.files, Section::Loading);
    assert!(
        s.update(SceneMsg::ToggleFiles, TAB).effects.is_empty(),
        "loading: no second load"
    );
    s.files_loaded(Ok(vec![]));
    assert!(matches!(s.files, Section::Ready { open: true, .. }));
    assert!(
        s.update(SceneMsg::ToggleFiles, TAB).effects.is_empty(),
        "closing loads nothing"
    );
    assert!(matches!(s.files, Section::Ready { open: false, .. }));
    assert!(
        s.update(SceneMsg::ToggleFiles, TAB).effects.is_empty(),
        "reopening loads nothing"
    );

    let effects = s.update(SceneMsg::ToggleGalleries, TAB).effects;
    assert_eq!(
        effects,
        vec![Effect::LoadSceneGalleries {
            tab: TAB,
            id: "7".into()
        }]
    );
    s.galleries_loaded(Err(AppError::NotConnected));
    assert!(matches!(s.galleries, Section::Failed(_)));
    assert_eq!(
        s.update(SceneMsg::ToggleGalleries, TAB).effects.len(),
        1,
        "a failure retries"
    );
}

fn chip(id: &str) -> ChipRef {
    ChipRef {
        kind: ChipKind::Performer,
        id: id.into(),
    }
}

fn rest_generation(effects: &[Effect]) -> u64 {
    match effects.first() {
        Some(Effect::ChipRestDue {
            generation, after, ..
        }) => {
            assert_eq!(*after, std::time::Duration::from_millis(400));
            *generation
        }
        other => panic!("a rest timer: {other:?}"),
    }
}

#[test]
fn a_chip_card_loads_only_after_a_rest() {
    let mut s = ready_state();
    let first = rest_generation(&s.update(SceneMsg::ChipEnter(chip("2")), TAB).effects);
    // Swept across: left before the rest.
    let _ = s.update(SceneMsg::ChipLeave(chip("2")), TAB);
    assert_eq!(s.chip_rested(first), None, "passed over: nothing loads");

    let second = rest_generation(&s.update(SceneMsg::ChipEnter(chip("3")), TAB).effects);
    assert!(
        s.update(SceneMsg::ChipEnter(chip("3")), TAB)
            .effects
            .is_empty(),
        "same chip: one timer"
    );
    assert_eq!(
        s.chip_rested(second),
        Some(chip("3")),
        "rested: load its card"
    );
    assert!(matches!(s.chip_hover, Some((_, _, true))));
    let _ = s.update(SceneMsg::CloseCard, TAB);
    assert!(s.chip_hover.is_none(), "Escape closes it");
}

#[test]
fn a_chip_card_is_fetched_once_per_session() {
    let mut library = semantic_stash_viewer_native::library::Library::default();
    assert_eq!(library.want_chip(ChipKind::Tag, "6").len(), 1);
    assert!(
        library.want_chip(ChipKind::Tag, "6").is_empty(),
        "on its way"
    );
    library.chip_loaded(
        ChipKind::Tag,
        "6".into(),
        Ok(stash_core::adapter::chips::ChipDetails::Tag {
            name: "A tag".into(),
            description: None,
            scene_count: 1,
        }),
    );
    assert!(
        library.want_chip(ChipKind::Tag, "6").is_empty(),
        "cached for the session"
    );
}

#[test]
fn choosing_a_marker_plays_from_it() {
    let mut s = ready_state();
    assert_eq!(
        s.update(SceneMsg::Marker(42.5), TAB).up,
        Some(SceneUp::PlayAt(42.5))
    );

    let (mut shell, _) = Shell::new();
    let _ = shell.update(ShellMsg::Open(Screen::scene("7", "Seven")));
    let up = shell.update(ShellMsg::Scene(SceneMsg::Marker(42.5))).up;
    assert_eq!(
        up,
        Some(Up::PlayAt {
            id: "7".into(),
            seconds: 42.5
        })
    );
}

#[test]
fn a_deleted_scene_says_it_no_longer_exists() {
    let mut s = SceneState::new("7", "Seven");
    let _ = s.enter(TAB);
    s.details_loaded(Err(AppError::SceneNotFound { id: "7".into() }));
    let Details::Failed(message) = &s.details else {
        panic!("failed")
    };
    assert!(message.contains("no longer exists"), "{message}");
}

#[test]
fn the_card_title_shows_before_the_details_and_stays_after() {
    let mut s = SceneState::new("7", "Seven (from the card)");
    let _ = s.enter(TAB);
    assert_eq!(
        s.title, "Seven (from the card)",
        "no request needed for the title"
    );
    s.details_loaded(Ok(details("7")));
    assert_eq!(s.title, "Seven");
}

#[test]
fn tag_chips_take_their_type_and_rule_colour_from_the_vocabulary() {
    use stash_core::semantic::{build_vocabulary, ColourRule, Rgb, SemanticKind};
    let mut rules = stash_core::semantic::default_colour_rules();
    rules.push(ColourRule {
        field: "ssvTestFlag".into(),
        colour: Rgb(1, 2, 3),
    });
    let vocabulary = build_vocabulary(
        &serde_json::json!({
            "isTrait": {"tags": [{"id": "9", "name": "T", "aliases": [], "custom_fields": {"isTrait": 1}}]},
            "variant": {"tags": [{"id": "11", "name": "V", "aliases": [], "custom_fields": {"variantOfTagId": "9"}}]},
            "rule_0": {"tags": [{"id": "13", "custom_fields": {"ssvTestFlag": 1}}]},
        }),
        &rules,
    );
    let trait_colour = rules
        .iter()
        .find(|r| r.field == "isTrait")
        .map(|r| r.colour);
    assert_eq!(vocabulary.kind_of("9"), Some(SemanticKind::Trait));
    assert_eq!(
        vocabulary.kind_of("11"),
        Some(SemanticKind::Trait),
        "a variant: its canonical's type"
    );
    assert_eq!(vocabulary.colour_of("11", &rules), trait_colour);
    assert_eq!(
        vocabulary.colour_of("13", &rules),
        Some(Rgb(1, 2, 3)),
        "an ordinary tag matched by an added rule"
    );
}

// ---- 008 T056a: performer pictures and portraits -------------------------------------------

use semantic_stash_viewer_native::screens::scenes::layout::Layout;
use semantic_stash_viewer_native::screens::scenes::picker::{performer_key, portrait_key};
use semantic_stash_viewer_native::screens::scenes::Thumbs;

#[test]
fn a_portrait_is_wanted_only_for_a_rested_performer_chip_with_an_image() {
    let mut s = ready_state();
    assert_eq!(s.portrait_wanted(), None, "no chip");
    let generation = rest_generation(&s.update(SceneMsg::ChipEnter(chip("2")), TAB).effects);
    assert_eq!(s.portrait_wanted(), None, "not rested yet");
    let _ = s.chip_rested(generation);
    assert_eq!(s.portrait_wanted(), Some(portrait_key("2", "1791406802")));
    let _ = s.update(SceneMsg::CloseCard, TAB);
    assert_eq!(s.portrait_wanted(), None, "card closed");

    // A tag chip, or a performer without an image, has no portrait.
    let tag = ChipRef {
        kind: ChipKind::Tag,
        id: "6".into(),
    };
    let generation = rest_generation(&s.update(SceneMsg::ChipEnter(tag), TAB).effects);
    let _ = s.chip_rested(generation);
    assert_eq!(s.portrait_wanted(), None);

    let mut d = details("7");
    d.performers[0].image = None;
    let mut s = SceneState::new("7", "Seven");
    let _ = s.enter(TAB);
    s.details_loaded(Ok(d));
    let generation = rest_generation(&s.update(SceneMsg::ChipEnter(chip("2")), TAB).effects);
    let _ = s.chip_rested(generation);
    assert_eq!(s.portrait_wanted(), None, "no image, no portrait");
}

#[test]
fn icon_and_portrait_are_separate_handles_from_one_cached_image() {
    use semantic_stash_viewer_native::effects::thumb_source;
    let (icon, portrait) = (performer_key("2", "7"), portrait_key("2", "7"));
    assert_ne!(icon, portrait, "separate handles");
    assert_eq!(
        (thumb_source(&icon), &icon.1),
        (thumb_source(&portrait), &portrait.1),
        "the same cached image (kind, id, version): no second download"
    );
    assert_eq!(thumb_source(&portrait), ("performer", "2"));

    let mut thumbs = Thumbs::default();
    let _ = thumbs.want([&icon], 80);
    thumbs.insert(icon.clone(), None);
    assert!(thumbs.get(&icon).is_some());
    assert!(
        thumbs.get(&portrait).is_none(),
        "the icon isn't the portrait"
    );
    assert_eq!(
        thumbs.want([&portrait], 480),
        vec![Effect::LoadThumbnails {
            keys: vec![portrait.clone()],
            width: 480
        }]
    );
    assert!(thumbs.want([&portrait], 480).is_empty(), "on its way");
}

#[test]
fn pictures_are_decoded_at_their_drawn_size_on_this_display() {
    use semantic_stash_viewer_native::screens::scene::{PERFORMER_ICON, PORTRAIT};
    let layout = Layout::new(1600.0, 1000.0);
    assert_eq!(layout.physical(PERFORMER_ICON), 40);
    let hidpi = layout.scaled(2.0);
    assert_eq!(hidpi.physical(PERFORMER_ICON), 80);
    assert_eq!(hidpi.physical(PORTRAIT), 480);
    assert_eq!(layout.scaled(1.25).physical(PERFORMER_ICON), 50);
    assert_eq!(layout.scaled(f32::NAN).scale, 1.0, "a bad scale is ignored");
    assert_eq!(layout.scaled(0.0).scale, 1.0);
}

#[test]
fn a_changed_picture_is_a_new_key_and_one_load() {
    use semantic_stash_viewer_native::effects::thumb_source;
    let (old, new) = (performer_key("2", "100"), performer_key("2", "200"));
    assert_ne!(old, new, "a new version never reuses the old handle");
    assert_eq!(thumb_source(&old), thumb_source(&new), "same performer");
    let mut thumbs = Thumbs::default();
    let _ = thumbs.want([&old], 80);
    thumbs.insert(old.clone(), None);
    assert_eq!(
        thumbs.want([&new], 80),
        vec![Effect::LoadThumbnails {
            keys: vec![new.clone()],
            width: 80
        }],
        "the new picture loads once"
    );
    assert!(thumbs.want([&new], 80).is_empty());
    // Icon and portrait at one version still share one cached picture.
    let portrait = portrait_key("2", "200");
    assert_eq!(
        (thumb_source(&new), &new.1),
        (thumb_source(&portrait), &portrait.1)
    );
}

/// 012 T015: an image's card thumbnail is keyed apart from a scene with the same id, and comes from
/// the core's `image` thumbnails.
#[test]
fn image_thumbnails_never_collide_with_scene_thumbnails() {
    use semantic_stash_viewer_native::effects::thumb_source;
    use semantic_stash_viewer_native::screens::scenes::thumbs::key_of;
    let scene = key_of("ssv-thumb://localhost/scene/5?v=1").expect("a scene key");
    let image = key_of("ssv-thumb://localhost/image/5?v=1").expect("an image key");
    assert_ne!(scene, image);
    assert_eq!(thumb_source(&scene), ("scene", "5"));
    assert_eq!(thumb_source(&image), ("image", "5"));
    assert_eq!(
        key_of("ssv-thumb://localhost/gallery/5?v=1"),
        None,
        "not served yet"
    );
}
