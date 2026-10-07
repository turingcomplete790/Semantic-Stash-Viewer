//! 008 US1: finding scenes. Search, criteria and chips, saved and default filters, the
//! pickers' library, semantic placeholders, scene-page chips, and the keyboard (FR-001 to
//! FR-008, FR-003f).

use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use semantic_stash_viewer_native::effects::{Effect, Reply};
use semantic_stash_viewer_native::screens::scenes::editor::EditorMsg;
use semantic_stash_viewer_native::screens::scenes::layout::Layout;
use semantic_stash_viewer_native::screens::scenes::state::SEARCH_ID;
use semantic_stash_viewer_native::screens::scenes::{Panel, ScenesMsg, ScenesState, ScenesUp};
use semantic_stash_viewer_native::screens::{Mode, Screen, Section};
use semantic_stash_viewer_native::session::{Msg, Session};
use semantic_stash_viewer_native::shell::ShellMsg;
use stash_core::adapter::choices::{Choice, ChoiceKind, ChoiceList};
use stash_core::profiles::ServerProfile;
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::scenes::filter::saved::{DisplayMode, SavedFilter};
use stash_core::scenes::filter::{Criterion, CriterionValue, Item, Modifier, SemanticFilter};
use stash_core::scenes::query::{SceneQuery, SceneSort};
use stash_core::scenes::ScenePage;

const TAB: u64 = 1;

fn layout() -> Layout {
    Layout::new(1100.0, 900.0)
}

fn loads(effects: &[Effect]) -> Vec<(u64, u32, SceneQuery)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::LoadScenesPage {
                generation,
                page,
                query,
                ..
            } => Some((*generation, *page, query.clone())),
            _ => None,
        })
        .collect()
}

fn empty_page(page: u32) -> ScenePage {
    ScenePage {
        count: 300,
        page,
        page_size: 50,
        items: vec![],
    }
}

/// A state showing page 3 of something.
fn ready() -> ScenesState {
    let mut s = ScenesState::at(3, 50);
    let effects = s.enter(TAB);
    let (generation, _, _) = loads(&effects)[0].clone();
    let _ = s.page_loaded(generation, Ok(empty_page(3)), false);
    s
}

fn organized() -> Criterion {
    Criterion::new(
        CriterionKind::Organized,
        Modifier::Equals,
        CriterionValue::Bool(true),
    )
}

fn performer(id: &str) -> Criterion {
    Criterion::new(
        CriterionKind::Performers,
        Modifier::IncludesAll,
        CriterionValue::Items {
            items: vec![Item {
                id: id.into(),
                label: format!("P{id}"),
            }],
            excluded: vec![],
            depth: None,
        },
    )
}

// ---- Search ------------------------------------------------------------------------------------

#[test]
fn typing_waits_then_one_search_runs_from_page_one() {
    let mut s = ready();
    let step = s.update(ScenesMsg::SearchInput("bea".into()), TAB, &layout());
    assert!(loads(&step.effects).is_empty(), "typing doesn't search yet");
    let Some(Effect::SearchDue {
        generation: first,
        after,
        ..
    }) = step.effects.first().cloned()
    else {
        panic!("a debounce: {:?}", step.effects)
    };
    assert_eq!(after, std::time::Duration::from_millis(300));
    assert_eq!(s.search_text(), "bea");
    let step = s.update(ScenesMsg::SearchInput("beach".into()), TAB, &layout());
    let Some(Effect::SearchDue {
        generation: second, ..
    }) = step.effects.first().cloned()
    else {
        panic!("a debounce")
    };
    assert!(
        s.search_due(first).is_empty(),
        "an older keystroke's timer does nothing"
    );
    let effects = s.search_due(second);
    let l = loads(&effects);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].1, 1, "back to page 1");
    assert_eq!(l[0].2.search, "beach");
}

#[test]
fn enter_searches_at_once_and_results_for_an_older_query_are_dropped() {
    let mut s = ready();
    let _ = s.update(ScenesMsg::SearchInput("x".into()), TAB, &layout());
    let first = loads(&s.update(ScenesMsg::SearchSubmit, TAB, &layout()).effects)[0].0;
    let _ = s.update(ScenesMsg::SearchInput("y".into()), TAB, &layout());
    let second = loads(&s.update(ScenesMsg::SearchSubmit, TAB, &layout()).effects)[0].0;
    assert!(s.page_loaded(first, Ok(empty_page(1)), false).is_empty());
    assert!(!s.ready(), "the older result didn't land");
    let _ = s.page_loaded(second, Ok(empty_page(1)), false);
    assert!(s.ready());
}

// ---- Criteria and chips ------------------------------------------------------------------------

fn apply_editor(s: &mut ScenesState) -> Vec<Effect> {
    s.update(ScenesMsg::Editor(EditorMsg::Apply), TAB, &layout())
        .effects
}

#[test]
fn adding_changing_removing_and_clearing_criteria_each_load_page_one() {
    let mut s = ready();
    let _ = s.update(ScenesMsg::Add(CriterionKind::Organized), TAB, &layout());
    let effects = apply_editor(&mut s);
    assert_eq!(loads(&effects).len(), 1);
    assert_eq!(s.page, 1);
    assert_eq!(s.query.criteria.len(), 1);

    let _ = s.update(ScenesMsg::Edit("organized".into()), TAB, &layout());
    let _ = s.update(ScenesMsg::Editor(EditorMsg::Flag(false)), TAB, &layout());
    assert_eq!(loads(&apply_editor(&mut s)).len(), 1);
    assert_eq!(
        s.query.criteria[0].value,
        CriterionValue::Bool(false),
        "changed in place"
    );

    let effects = s
        .update(ScenesMsg::Remove("organized".into()), TAB, &layout())
        .effects;
    assert_eq!(loads(&effects).len(), 1);
    assert!(s.query.criteria.is_empty());

    s.query.set_criterion(organized());
    let effects = s.update(ScenesMsg::ClearFilters, TAB, &layout()).effects;
    assert_eq!(loads(&effects).len(), 1);
    assert!(s.query.criteria.is_empty());
}

#[test]
fn an_unusable_criterion_says_why_and_isnt_added() {
    let mut s = ready();
    let _ = s.update(ScenesMsg::Add(CriterionKind::Title), TAB, &layout());
    assert!(loads(&apply_editor(&mut s)).is_empty());
    let Panel::Editor(editor) = &s.panel else {
        panic!("the editor stays open")
    };
    assert!(editor.error.is_some());
}

#[test]
fn a_picker_kind_asks_the_library_for_its_list() {
    let mut s = ready();
    let step = s.update(ScenesMsg::Add(CriterionKind::Tags), TAB, &layout());
    assert_eq!(
        step.up,
        Some(ScenesUp::Need(
            semantic_stash_viewer_native::library::Need::Choices(ChoiceKind::Tags)
        ))
    );
}

#[test]
fn semantic_placeholders_open_nothing_and_survive_a_restore() {
    let mut s = ready();
    let kind = CriterionKind::Semantic(SemanticFilter::Role);
    let step = s.update(ScenesMsg::Add(kind), TAB, &layout());
    assert!(step.effects.is_empty() && step.up.is_none());
    assert_eq!(s.panel, Panel::None);

    s.query.criteria = vec![Criterion::new(kind, Modifier::Equals, CriterionValue::None)];
    let json = serde_json::to_string(&s.persistent()).expect("save");
    let back: ScenesState = serde_json::from_str(&json).expect("restore");
    assert_eq!(back.query.criteria, s.query.criteria);
}

// ---- Saved and default filters -----------------------------------------------------------------

fn saved(left_out: Vec<String>) -> SavedFilter {
    SavedFilter {
        id: "2".into(),
        name: "Favourites".into(),
        query: SceneQuery {
            search: "beach".into(),
            sort: SceneSort::Title,
            criteria: vec![performer("3")],
            ..SceneQuery::default()
        },
        page_size: Some(40),
        mode: Some(DisplayMode::List),
        left_out,
    }
}

#[test]
fn applying_a_saved_filter_sets_everything_and_loads_once() {
    let mut s = ready();
    let effects = s
        .update(
            ScenesMsg::ApplySaved(Box::new(saved(vec![]))),
            TAB,
            &layout(),
        )
        .effects;
    let l = loads(&effects);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].1, 1);
    assert_eq!(s.query.search, "beach");
    assert_eq!(s.query.sort, SceneSort::Title);
    assert_eq!(s.page_size, 40);
    assert_eq!(s.mode, Mode::List);
    assert_eq!(
        s.applied.as_ref().map(|a| a.name.as_str()),
        Some("Favourites")
    );

    let _ = s.update(ScenesMsg::Remove("performers".into()), TAB, &layout());
    assert!(
        s.applied.is_none(),
        "changing the criteria clears the applied name"
    );
}

#[test]
fn a_page_size_the_viewer_doesnt_offer_takes_the_nearest() {
    let mut f = saved(vec![]);
    f.page_size = Some(100);
    assert_eq!(ScenesState::from_saved(&f).page_size, 120);
}

#[test]
fn a_saved_filter_with_parts_left_out_says_so() {
    let mut s = ready();
    let _ = s.update(
        ScenesMsg::ApplySaved(Box::new(saved(vec!["future_field".into()]))),
        TAB,
        &layout(),
    );
    assert_eq!(s.left_out, vec!["future_field"]);
    let _ = s.update(ScenesMsg::DismissLeftOut, TAB, &layout());
    assert!(s.left_out.is_empty());
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

fn session() -> Session {
    let p = profile();
    let (mut s, effects) = Session::enter(p.clone(), true);
    assert!(
        effects.contains(&Effect::LoadUiSettings),
        "UI settings read at the start"
    );
    let _ = s.update(Msg::Reply(Reply::SessionLoaded {
        profile: p.id,
        saved: None,
    }));
    s
}

fn scenes(s: &Session) -> &ScenesState {
    match s.shell.active().current() {
        Screen::Scenes(state) => state,
        other => panic!("not Scenes: {other:?}"),
    }
}

#[test]
fn a_new_scenes_tab_starts_from_the_servers_default_filter() {
    let mut s = session();
    let ui: stash_core::adapter::filters::UiSettings =
        stash_core::adapter::filters::parse_ui_settings(
            &serde_json::from_str(
                r#"{"defaultFilters":{"scenes":{
            "find_filter":{"sort":"title","direction":"ASC","per_page":40},
            "object_filter":{"organized":{"value":"true","modifier":"EQUALS"}}}}}"#,
            )
            .expect("json"),
        );
    let _ = s.update(Msg::Reply(Reply::UiSettings(Ok(ui))));
    let _ = s.update(Msg::Shell(ShellMsg::Section(Section::Scenes)));
    let state = scenes(&s);
    assert_eq!(state.query.sort, SceneSort::Title);
    assert_eq!(state.query.criteria, vec![organized()]);
    assert!(
        state.applied.is_none(),
        "the default isn't shown as an applied saved filter"
    );
}

fn tags_list() -> ChoiceList {
    ChoiceList {
        kind: ChoiceKind::Tags,
        entries: (1..=3)
            .map(|i| Choice {
                id: i.to_string(),
                name: format!("Tag {i}"),
                aliases: vec![],
            })
            .collect(),
    }
}

#[test]
fn a_picker_list_is_read_once_per_session() {
    let mut s = session();
    let _ = s.update(Msg::Shell(ShellMsg::Section(Section::Scenes)));
    let first = s
        .update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Add(
            CriterionKind::Tags,
        ))))
        .effects;
    assert!(first.contains(&Effect::LoadChoices(ChoiceKind::Tags)));
    assert!(
        first.iter().any(|e| matches!(e, Effect::LoadVocabulary(_))),
        "the Tags picker also needs the vocabulary"
    );
    let _ = s.update(Msg::Reply(Reply::Choices {
        kind: ChoiceKind::Tags,
        result: Ok(tags_list()),
    }));
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Editor(
        EditorMsg::Cancel,
    ))));
    let again = s
        .update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Add(
            CriterionKind::Tags,
        ))))
        .effects;
    assert!(
        !again
            .iter()
            .any(|e| matches!(e, Effect::LoadChoices(_) | Effect::LoadVocabulary(_))),
        "cached: no second read: {again:?}"
    );
}

// ---- Keyboard (FR-003f) ------------------------------------------------------------------------

fn key(s: &mut Session, key: Key, mods: Modifiers) -> Vec<Effect> {
    s.update(Msg::Key(key, mods)).effects
}

fn named(n: Named) -> Key {
    Key::Named(n)
}

fn ch(c: &str) -> Key {
    Key::Character(c.into())
}

#[test]
fn the_filters_work_without_a_pointer() {
    let mut s = session();
    let _ = s.update(Msg::Shell(ShellMsg::Section(Section::Scenes)));

    assert!(key(&mut s, ch("/"), Modifiers::empty()).contains(&Effect::Focus(SEARCH_ID)));

    let _ = key(&mut s, ch("f"), Modifiers::empty());
    assert!(matches!(scenes(&s).panel, Panel::Menu { .. }));
    // The first kind is Title; the second is Studio code.
    let _ = key(&mut s, named(Named::ArrowDown), Modifiers::empty());
    let _ = key(&mut s, named(Named::Enter), Modifiers::empty());
    let Panel::Editor(editor) = &scenes(&s).panel else {
        panic!("Enter opens the highlighted kind's editor")
    };
    assert_eq!(editor.kind, CriterionKind::Code);
    let _ = key(&mut s, named(Named::Escape), Modifiers::empty());
    assert_eq!(scenes(&s).panel, Panel::None);

    // The picker: arrows move, Enter includes, Ctrl+E excludes.
    let _ = s.update(Msg::Shell(ShellMsg::Scenes(ScenesMsg::Add(
        CriterionKind::Tags,
    ))));
    let _ = s.update(Msg::Reply(Reply::Choices {
        kind: ChoiceKind::Tags,
        result: Ok(tags_list()),
    }));
    let _ = s.update(Msg::Reply(Reply::Vocabulary(Ok(
        stash_core::semantic::SemanticVocabulary::default(),
    ))));
    let _ = key(&mut s, named(Named::ArrowDown), Modifiers::empty());
    let _ = key(&mut s, named(Named::Enter), Modifiers::empty());
    let _ = key(&mut s, named(Named::ArrowDown), Modifiers::empty());
    let _ = key(&mut s, ch("e"), Modifiers::CTRL);
    let Panel::Editor(editor) = &scenes(&s).panel else {
        panic!("the picker is open")
    };
    assert_eq!(
        editor
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["2"]
    );
    assert_eq!(
        editor
            .excluded
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["3"]
    );
    let _ = key(&mut s, named(Named::Enter), Modifiers::CTRL);
    assert_eq!(scenes(&s).query.criteria.len(), 1, "Ctrl+Enter applies");

    // The menu lists the active filter first: Delete removes it.
    let _ = key(&mut s, ch("f"), Modifiers::empty());
    let _ = key(&mut s, named(Named::Delete), Modifiers::empty());
    assert!(scenes(&s).query.criteria.is_empty());
}

// ---- Scene page chips (FR-006) -----------------------------------------------------------------

#[test]
fn a_chip_opens_scenes_filtered_to_it_here_or_with_ctrl_in_a_new_tab() {
    let mut s = session();
    let tabs = s.shell.tabs.len();
    let _ = s.update(Msg::Shell(ShellMsg::FilterTo {
        criterion: performer("9"),
        new_tab: false,
    }));
    assert_eq!(s.shell.tabs.len(), tabs);
    assert_eq!(scenes(&s).query.criteria, vec![performer("9")]);

    let _ = s.update(Msg::Modifiers(Modifiers::CTRL));
    let _ = s.update(Msg::Shell(ShellMsg::FilterTo {
        criterion: performer("8"),
        new_tab: false,
    }));
    assert_eq!(s.shell.tabs.len(), tabs + 1, "Ctrl+click: a new tab");
    assert_eq!(scenes(&s).query.criteria, vec![performer("8")]);
}

#[test]
fn no_matches_offers_to_clear_the_filters() {
    let mut s = ready();
    s.query.set_criterion(organized());
    let effects = s.update(ScenesMsg::ClearFilters, TAB, &layout()).effects;
    assert_eq!(loads(&effects).len(), 1);
    assert!(s.query.criteria.is_empty() && s.query.search.is_empty());
}
