//! 007 T034: the Scenes screen's transitions (spec US3; behaviours that stay B1, B3).

use semantic_stash_viewer_native::effects::Effect;
use semantic_stash_viewer_native::screens::scenes::layout::Layout;
use semantic_stash_viewer_native::screens::scenes::{Data, ScenesMsg, ScenesState};
use semantic_stash_viewer_native::screens::Mode;
use stash_core::scenes::query::SceneSort;
use stash_core::scenes::{SceneCard, ScenePage};
use stash_core::AppError;

const TAB: u64 = 1;

fn layout() -> Layout {
    // 4 grid columns at this width.
    Layout::new(1100.0, 900.0)
}

fn cards(page: u32, size: u32, count: u32) -> Vec<SceneCard> {
    let first = (page - 1) * size;
    (first..(first + size).min(count))
        .map(|i| SceneCard {
            id: (i + 1).to_string(),
            title: format!("Scene {}", i + 1),
            date: None,
            duration_seconds: Some(60.0),
            resolution: None,
            studio: None,
            thumb: Some(format!("ssv-thumb://localhost/scene/{}?v=1", i + 1)),
            has_preview: false,
        })
        .collect()
}

fn page(page: u32, size: u32, count: u32) -> ScenePage {
    ScenePage {
        count,
        page,
        page_size: size,
        items: cards(page, size, count),
    }
}

fn loads(effects: &[Effect]) -> Vec<(u64, u32, u32)> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::LoadScenesPage {
                generation,
                page,
                size,
                ..
            } => Some((*generation, *page, *size)),
            _ => None,
        })
        .collect()
}

/// A state showing `p` of a `count`-scene library.
fn ready(p: u32, size: u32, count: u32) -> ScenesState {
    let mut s = ScenesState::at(p, size);
    let effects = s.enter(TAB);
    let (generation, _, _) = loads(&effects)[0];
    let _ = s.page_loaded(generation, Ok(page(p, size, count)), false);
    s
}

#[test]
fn entering_loads_once_and_a_ready_page_doesnt_reload() {
    let mut s = ScenesState::default();
    let effects = s.enter(TAB);
    let l = loads(&effects);
    assert_eq!(l.len(), 1);
    assert_eq!((l[0].1, l[0].2), (1, 50), "page 1, 50 per page by default");
    assert!(matches!(s.data, Data::Loading));
    let _ = s.page_loaded(l[0].0, Ok(page(1, 50, 120)), false);
    assert!(matches!(s.data, Data::Ready { .. }));
    assert!(loads(&s.enter(TAB)).is_empty());
}

#[test]
fn results_for_an_older_request_are_dropped() {
    let mut s = ScenesState::default();
    let first = loads(&s.enter(TAB))[0].0;
    let step = s.update(ScenesMsg::Page(2), TAB, &layout());
    let second = loads(&step.effects)[0].0;
    assert_ne!(first, second);
    let _ = s.page_loaded(first, Ok(page(1, 50, 500)), false);
    assert!(matches!(s.data, Data::Loading), "the old page never shows");
    let _ = s.page_loaded(second, Ok(page(2, 50, 500)), false);
    assert!(matches!(&s.data, Data::Ready { cards, .. } if cards[0].id == "51"));
}

#[test]
fn a_page_past_the_end_shows_the_last_page() {
    let mut s = ScenesState::at(9, 50);
    let g = loads(&s.enter(TAB))[0].0;
    // The core answers with the last page that exists.
    let _ = s.page_loaded(g, Ok(page(4, 50, 190)), false);
    assert_eq!(s.page, 4);
}

#[test]
fn page_moves_stay_in_range() {
    let mut s = ready(4, 50, 190);
    assert!(loads(&s.update(ScenesMsg::Page(5), TAB, &layout()).effects).is_empty());
    assert_eq!(s.page, 4);
    let step = s.update(ScenesMsg::Page(0), TAB, &layout());
    assert_eq!(loads(&step.effects)[0].1, 1);
}

#[test]
fn a_new_sort_goes_to_page_one() {
    let mut s = ready(3, 50, 500);
    let step = s.update(ScenesMsg::Sort(SceneSort::Title), TAB, &layout());
    assert_eq!(s.page, 1);
    assert_eq!(s.query.sort, SceneSort::Title);
    assert_eq!(loads(&step.effects)[0].1, 1);
}

#[test]
fn random_gets_a_seed() {
    let mut s = ready(1, 50, 500);
    let _ = s.update(ScenesMsg::Sort(SceneSort::Random), TAB, &layout());
    assert!(s.query.seed.is_some());
    let seed = s.query.seed;
    let _ = s.update(ScenesMsg::Reshuffle, TAB, &layout());
    assert_ne!(s.query.seed, seed);
}

#[test]
fn a_new_page_size_keeps_the_first_visible_scene() {
    // Page 3 of 50 at the top: scene index 100 is first; at 20 per page that's page 6.
    let mut s = ready(3, 50, 500);
    let _ = s.update(ScenesMsg::PageSize(20), TAB, &layout());
    assert_eq!((s.page, s.page_size), (6, 20));

    // Scrolled two rows down (4 columns): index 108; at 120 per page that's page 1.
    let mut s = ready(3, 50, 500);
    let l = layout();
    s.scroll = 2.0 * l.row_height(Mode::Grid) + 1.0;
    let _ = s.update(ScenesMsg::PageSize(120), TAB, &l);
    assert_eq!((s.page, s.page_size), (1, 120));
}

#[test]
fn only_the_offered_page_sizes_are_accepted() {
    let mut s = ready(1, 50, 500);
    assert!(s
        .update(ScenesMsg::PageSize(33), TAB, &layout())
        .effects
        .is_empty());
    assert_eq!(s.page_size, 50);
    assert_eq!(ScenesState::default().page_size, 50);
}

#[test]
fn a_ready_page_prefetches_its_neighbours_and_warms_the_next() {
    let mut s = ScenesState::at(3, 50);
    let g = loads(&s.enter(TAB))[0].0;
    let effects = s.page_loaded(g, Ok(page(3, 50, 500)), false);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::PrefetchScenes { pages, size: 50, warm: Some(4), .. } if pages == &vec![4, 2]
    )));
    // On the last page, nothing after it.
    let mut s = ScenesState::at(10, 50);
    let g = loads(&s.enter(TAB))[0].0;
    let effects = s.page_loaded(g, Ok(page(10, 50, 500)), false);
    assert!(effects.iter().any(|e| matches!(
        e,
        Effect::PrefetchScenes { pages, warm: None, .. } if pages == &vec![9]
    )));
}

#[test]
fn failures_show_unreachable_unless_still_connecting() {
    let mut s = ScenesState::default();
    let g = loads(&s.enter(TAB))[0].0;
    let _ = s.page_loaded(g, Err(AppError::NotConnected), true);
    assert!(
        matches!(s.data, Data::Loading),
        "still connecting: keep waiting"
    );
    assert!(s.needs_load());
    let g = loads(&s.enter(TAB))[0].0;
    let _ = s.page_loaded(g, Err(AppError::NotConnected), false);
    assert!(matches!(s.data, Data::Unreachable));
}

#[test]
fn keyboard_focus_crosses_pages() {
    let mut s = ready(2, 50, 500);
    let l = layout();
    let _ = s.update(ScenesMsg::Focus(49), TAB, &l);
    let step = s.update(
        ScenesMsg::Grid(
            semantic_stash_viewer_native::screens::scenes::keyboard::GridAction::Edge(
                semantic_stash_viewer_native::screens::scenes::keyboard::PageDirection::Next,
            ),
        ),
        TAB,
        &l,
    );
    assert_eq!(s.page, 3);
    assert_eq!(s.focused, Some(0), "the first card of the next page");
    assert_eq!(loads(&step.effects).len(), 1);
}

#[test]
fn a_new_page_starts_at_the_top() {
    let mut s = ready(2, 50, 500);
    s.scroll = 900.0;
    let _ = s.update(ScenesMsg::Page(3), TAB, &layout());
    assert_eq!(s.scroll, 0.0);
}

#[test]
fn a_cache_refresh_reloads_in_place() {
    let mut s = ready(2, 50, 500);
    let key = s.cache_key();
    let effects = s.cache_changed(&key, TAB);
    assert_eq!(loads(&effects).len(), 1);
    assert!(
        matches!(s.data, Data::Ready { .. }),
        "cards stay while it reloads"
    );
    assert!(s.cache_changed("scenes:q:other:s:50:p:2", TAB).is_empty());
}
