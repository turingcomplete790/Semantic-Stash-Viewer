//! 007 T055: the Settings screen (spec US6; capabilities C4, C13).

use semantic_stash_viewer_native::effects::Effect;
use semantic_stash_viewer_native::screens::settings::{
    Clear, Editor, ServersMsg, SettingsMsg, SettingsState, TroubleMsg,
};
use semantic_stash_viewer_native::screens::SettingsPage;
use stash_core::profiles::ServerProfile;
use stash_core::AppError;

fn profile(name: &str, port: u16) -> ServerProfile {
    ServerProfile {
        id: uuid::Uuid::new_v4(),
        display_name: name.into(),
        base_url: format!("http://localhost:{port}/").parse().expect("url"),
        strict_tls: false,
        api_key: Some("secret".into()),
        created_at: chrono::Utc::now(),
        last_used_at: None,
    }
}

fn servers(list: &[ServerProfile]) -> SettingsState {
    let mut s = SettingsState::at(SettingsPage::Servers);
    let effects = s.enter();
    assert!(
        effects.contains(&Effect::LoadProfiles),
        "Servers lists the profiles"
    );
    s.servers.loaded(list.to_vec());
    s
}

fn send(s: &mut SettingsState, m: ServersMsg) -> Vec<Effect> {
    s.update(SettingsMsg::Servers(m)).effects
}

#[test]
fn the_editor_tests_before_saving_and_explains_a_failure() {
    let (a, b) = (profile("Production", 9999), profile("Testing", 9998));
    let mut s = servers(&[a.clone(), b]);
    let _ = send(&mut s, ServersMsg::Edit(a.id));
    let Editor::Editing { form, .. } = &s.servers.editor else {
        panic!("editing")
    };
    assert_eq!(form.name, "Production");
    assert_eq!(form.api_key, "secret", "the key is kept unless changed");

    let _ = send(&mut s, ServersMsg::Address("localhost:9997".into()));
    let effects = send(&mut s, ServersMsg::Save);
    assert!(matches!(s.servers.editor, Editor::Testing { .. }));
    let Some(Effect::SaveProfile { id, draft, force }) = effects.first() else {
        panic!("{effects:?}")
    };
    assert_eq!(
        (*id, draft.address.as_str(), *force),
        (a.id, "localhost:9997", false)
    );

    s.servers.saved(a.id, Err(AppError::NotConnected));
    let Editor::Error { message, .. } = &s.servers.editor else {
        panic!("error shown")
    };
    assert!(!message.is_empty());

    // Save anyway (the server may be down for now).
    let effects = send(&mut s, ServersMsg::SaveAnyway);
    assert!(matches!(
        effects.first(),
        Some(Effect::SaveProfile { force: true, .. })
    ));
    let saved = ServerProfile {
        base_url: "http://localhost:9997/".parse().expect("url"),
        ..a.clone()
    };
    s.servers.saved(a.id, Ok(saved));
    assert!(matches!(s.servers.editor, Editor::Closed));
    assert!(s.servers.list[0].base_url.as_str().contains("9997"));
}

#[test]
fn cancel_leaves_the_server_unchanged() {
    let a = profile("Production", 9999);
    let mut s = servers(std::slice::from_ref(&a));
    let _ = send(&mut s, ServersMsg::Edit(a.id));
    let _ = send(&mut s, ServersMsg::Name("Changed".into()));
    assert!(send(&mut s, ServersMsg::Cancel).is_empty());
    assert!(matches!(s.servers.editor, Editor::Closed));
    assert_eq!(s.servers.list[0].display_name, "Production");
}

#[test]
fn delete_asks_first() {
    let (a, b) = (profile("Production", 9999), profile("Testing", 9998));
    let mut s = servers(&[a, b.clone()]);
    assert!(send(&mut s, ServersMsg::Delete(b.id)).is_empty());
    assert_eq!(s.servers.confirm_delete, Some(b.id));
    assert!(send(&mut s, ServersMsg::CancelDelete).is_empty());
    assert_eq!(s.servers.confirm_delete, None);
    let _ = send(&mut s, ServersMsg::Delete(b.id));
    assert_eq!(
        send(&mut s, ServersMsg::ConfirmDelete),
        vec![Effect::DeleteProfile(b.id)]
    );
}

#[test]
fn reordering_moves_and_saves_the_order() {
    let (a, b, c) = (profile("A", 9001), profile("B", 9002), profile("C", 9003));
    let mut s = servers(&[a.clone(), b.clone(), c.clone()]);
    let effects = send(&mut s, ServersMsg::MoveUp(c.id));
    assert_eq!(
        effects,
        vec![Effect::ReorderProfiles(vec![a.id, c.id, b.id])]
    );
    assert!(
        send(&mut s, ServersMsg::MoveUp(a.id)).is_empty(),
        "already first"
    );
    let effects = send(&mut s, ServersMsg::MoveDown(a.id));
    assert_eq!(
        effects,
        vec![Effect::ReorderProfiles(vec![c.id, a.id, b.id])]
    );
}

#[test]
fn troubleshooting_reads_the_cache_size_and_clears_it() {
    let mut s = SettingsState::at(SettingsPage::Troubleshooting);
    assert!(s.enter().contains(&Effect::ReadCacheSize));
    s.troubleshooting.cache_size = Some(4096);
    let effects = s.update(SettingsMsg::Trouble(TroubleMsg::Clear)).effects;
    assert_eq!(effects, vec![Effect::ClearCache]);
    assert!(matches!(s.troubleshooting.clear, Clear::Clearing));
    s.troubleshooting.cleared(Ok(4096));
    assert!(matches!(s.troubleshooting.clear, Clear::Cleared(4096)));
    assert_eq!(s.troubleshooting.cache_size, Some(0));
    let effects = s.update(SettingsMsg::Trouble(TroubleMsg::OpenLogs)).effects;
    assert_eq!(effects, vec![Effect::OpenLogFolder]);
}

#[test]
fn switching_pages_runs_their_entry() {
    let mut s = SettingsState::at(SettingsPage::Keyboard);
    assert!(s.enter().is_empty());
    let effects = s.open(SettingsPage::Troubleshooting);
    assert!(effects.contains(&Effect::ReadCacheSize));
    let effects = s.open(SettingsPage::Servers);
    assert!(effects.contains(&Effect::LoadProfiles));
}

#[test]
fn keyboard_lists_the_keymap() {
    use semantic_stash_viewer_native::screens::scenes::layout::Layout;
    use semantic_stash_viewer_native::screens::scenes::Thumbs;
    use semantic_stash_viewer_native::screens::Context;
    use semantic_stash_viewer_native::session::connection::Connection;
    let s = SettingsState::at(SettingsPage::Keyboard);
    let p = profile("Testing", 9998);
    let (connection, thumbs) = (Connection::default(), Thumbs::default());
    let ctx = Context {
        profile: &p,
        connection: &connection,
        thumbs: &thumbs,
        library: &semantic_stash_viewer_native::library::Library::default(),
        layout: Layout::new(1400.0, 1000.0),
        playback: Default::default(),
        previews: &Default::default(),
    };
    let mut ui = iced_test::simulator(s.view(&ctx));
    for label in [
        "New tab",
        "Keyboard shortcuts",
        "Play or pause",
        "Next page",
    ] {
        assert!(ui.find(label).is_ok(), "{label} is listed");
    }
}

// ---- 008 T050: Settings → Tag colours --------------------------------------------------------

mod tag_colours {
    use semantic_stash_viewer_native::effects::Effect;
    use semantic_stash_viewer_native::screens::settings::tag_colours::{
        Saving, TagColoursMsg as M, TagColoursPage,
    };
    use stash_core::semantic::{default_colour_rules, Rgb};

    fn page() -> TagColoursPage {
        let mut p = TagColoursPage::default();
        p.sync(default_colour_rules());
        p
    }

    #[test]
    fn changing_a_colour_and_saving_is_one_write_of_every_rule() {
        let mut p = page();
        let _ = p.update(M::Edit(2));
        let _ = p.update(M::SetColour(2, Rgb(1, 2, 3)));
        assert!(p.dirty);
        let effects = p.update(M::Save);
        let [Effect::SaveTagColours(rules)] = effects.as_slice() else {
            panic!("one save: {effects:?}")
        };
        assert_eq!(rules.len(), 7);
        assert_eq!(rules[2].colour, Rgb(1, 2, 3));
        assert_eq!(p.saving, Saving::Saving);
        assert!(p.update(M::Save).is_empty(), "no second write while saving");
        p.saved(Ok(rules.clone()));
        assert!(!p.dirty);
        assert_eq!(p.saving, Saving::Saved);
    }

    #[test]
    fn a_failed_save_says_so_and_keeps_the_edits() {
        let mut p = page();
        let _ = p.update(M::Reset);
        let _ = p.update(M::SetColour(0, Rgb(9, 9, 9)));
        let _ = p.update(M::Save);
        p.saved(Err("Can't reach the server".into()));
        assert!(matches!(p.saving, Saving::Failed(_)));
        assert!(p.dirty, "the edits stay to try again");
        assert_eq!(p.rules.as_ref().expect("rules")[0].colour, Rgb(9, 9, 9));
    }

    #[test]
    fn rules_can_be_added_reordered_and_only_added_ones_removed() {
        let mut p = page();
        let _ = p.update(M::NewField("bad name".into()));
        let _ = p.update(M::AddRule);
        assert!(p.error.is_some(), "field names are checked");
        let _ = p.update(M::NewField("isRole".into()));
        let _ = p.update(M::AddRule);
        assert!(p.error.is_some(), "one rule per field");

        let _ = p.update(M::NewField("ssvTestFlag".into()));
        let _ = p.update(M::AddRule);
        let rules = p.rules.clone().expect("rules");
        assert_eq!(rules.len(), 8);
        assert_eq!(rules[7].field, "ssvTestFlag");

        let _ = p.update(M::MoveUp(7));
        assert_eq!(p.rules.as_ref().expect("rules")[6].field, "ssvTestFlag");
        let _ = p.update(M::Remove(0));
        assert_eq!(
            p.rules.as_ref().expect("rules").len(),
            8,
            "a semantic default stays"
        );
        let _ = p.update(M::Remove(6));
        assert_eq!(
            p.rules.as_ref().expect("rules").len(),
            7,
            "an added rule goes"
        );
        let _ = p.update(M::Reset);
        assert_eq!(p.rules, Some(default_colour_rules()));
    }

    #[test]
    fn a_hex_colour_is_checked() {
        let mut p = page();
        let _ = p.update(M::Edit(1));
        let _ = p.update(M::Hex("green".into()));
        let _ = p.update(M::ApplyHex);
        assert!(p.error.is_some());
        let _ = p.update(M::Hex("#ABCDEF".into()));
        let _ = p.update(M::ApplyHex);
        assert_eq!(
            p.rules.as_ref().expect("rules")[1].colour,
            Rgb(0xab, 0xcd, 0xef)
        );
    }

    #[test]
    fn the_servers_rules_dont_overwrite_edits_in_progress() {
        let mut p = page();
        let _ = p.update(M::SetColour(0, Rgb(5, 5, 5)));
        p.sync(default_colour_rules());
        assert_eq!(p.rules.as_ref().expect("rules")[0].colour, Rgb(5, 5, 5));
    }

    #[test]
    fn the_keyboard_moves_the_highlight_within_the_rules() {
        let mut p = page();
        let _ = p.update(M::Highlight(-1));
        assert_eq!(p.highlight, 0);
        for _ in 0..20 {
            let _ = p.update(M::Highlight(1));
        }
        assert_eq!(p.highlight, 6);
    }
}
