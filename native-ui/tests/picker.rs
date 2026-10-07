//! The picker's view across frames (008 FR-003a). A Tags picker's rows change shape when the
//! vocabulary arrives (type headers appear above the tags); building the view again with the same
//! widget tree must not reuse a header's state for a tag row ("Downcast widget state" panic).

use std::sync::Arc;

use iced::advanced::renderer::Headless;
use iced::{Font, Pixels, Size};
use iced_runtime::user_interface::{Cache, UserInterface};
use semantic_stash_viewer_native::library::{Choices, Library, Loadable};
use semantic_stash_viewer_native::screens::scenes::editor::Editor;
use semantic_stash_viewer_native::screens::scenes::{picker, Thumbs};
use stash_core::adapter::choices::{Choice, ChoiceKind, ChoiceList};
use stash_core::scenes::filter::kinds::CriterionKind;
use stash_core::semantic::{build_vocabulary, default_colour_rules};

fn tags() -> Arc<Choices> {
    let list = ChoiceList {
        kind: ChoiceKind::Tags,
        entries: (1..=40)
            .map(|i| Choice {
                id: i.to_string(),
                name: format!("Tag {i}"),
                aliases: vec![],
            })
            .collect(),
    };
    let folded = list.folded();
    Arc::new(Choices { list, folded })
}

#[test]
fn the_tags_picker_survives_the_vocabulary_arriving() {
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut renderer = runtime
        .block_on(iced::Renderer::new(Font::DEFAULT, Pixels(16.0), None))
        .expect("headless renderer");
    let size = Size::new(800.0, 600.0);
    let thumbs = Thumbs::default();
    let editor = Editor::new(CriterionKind::Tags);

    let mut library = Library::default();
    library
        .choices
        .insert(ChoiceKind::Tags, Loadable::Ready(tags()));
    let ui = UserInterface::build(
        picker::view(&library, &editor, &thumbs),
        size,
        Cache::default(),
        &mut renderer,
    );
    let cache = ui.into_cache();

    // The vocabulary arrives: headers now sit where tag rows were.
    let vocabulary = build_vocabulary(
        &serde_json::json!({
            "isTrait": {"tags": [{"id": "100", "name": "Brave", "aliases": [], "custom_fields": {"isTrait": 1}}]},
            "isRole": {"tags": [{"id": "101", "name": "Hero", "aliases": [], "custom_fields": {"isRole": 1}}]},
        }),
        &default_colour_rules(),
    );
    library.vocabulary = Loadable::Ready(Arc::new(vocabulary));
    let ui = UserInterface::build(
        picker::view(&library, &editor, &thumbs),
        size,
        cache,
        &mut renderer,
    );
    let cache = ui.into_cache();

    // And back again (the vocabulary refreshed away): still no panic.
    library.vocabulary = Loadable::NotLoaded;
    let _ = UserInterface::build(
        picker::view(&library, &editor, &thumbs),
        size,
        cache,
        &mut renderer,
    );
}
