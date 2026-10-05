//! 007 T052: the notification centre in the shell (spec US5; capability C12; behaviour that stays
//! B7). The core's `NotificationCenter` is real (in a temporary folder); the UI state is fed from
//! it the way the app's subscription feeds it.

use std::time::Duration;

use semantic_stash_viewer_native::effects::{Effect, NotificationAction};
use semantic_stash_viewer_native::shell::notifications::{Msg, Notifications};
use stash_core::shell::notifications::{
    JobProgress, JobStatus, NewNotification, NotificationCenter, NotificationKind, Severity,
};

fn center() -> (tempfile::TempDir, NotificationCenter) {
    let dir = tempfile::tempdir().expect("tempdir");
    let center = NotificationCenter::open(dir.path().join("notifications.json"));
    (dir, center)
}

fn note(key: &str, severity: Severity, title: &str, toast: bool) -> NewNotification {
    NewNotification {
        key: Some(key.into()),
        profile_id: None,
        kind: NotificationKind::Connection,
        severity,
        title: title.into(),
        detail: Some("Details.".into()),
        toast,
        job: None,
    }
}

fn job(status: JobStatus, progress: Option<f64>) -> NewNotification {
    NewNotification {
        kind: NotificationKind::Job,
        job: Some(JobProgress {
            status,
            progress,
            started_at: None,
            ended_at: None,
        }),
        ..note("job:7", Severity::Info, "Scanning", false)
    }
}

/// Apply what the UI asked the centre to do, as the effect runner does.
fn apply(center: &NotificationCenter, effects: &[Effect]) {
    for e in effects {
        if let Effect::Notifications(action) = e {
            match action {
                NotificationAction::MarkRead(ids) => center.mark_read(ids),
                NotificationAction::Dismiss(id) => center.dismiss(id),
                NotificationAction::DismissAll => center.dismiss_all(),
            }
        }
    }
}

fn sync(ui: &mut Notifications, center: &NotificationCenter) -> Vec<Effect> {
    ui.update(Msg::Changed(center.list()))
}

#[test]
fn a_lost_connection_is_one_entry_that_updates_when_it_returns() {
    let (_dir, center) = center();
    let mut ui = Notifications::default();
    center.post(note(
        "connection:a",
        Severity::Error,
        "Server unreachable",
        true,
    ));
    let _ = sync(&mut ui, &center);
    center.post(note("connection:a", Severity::Info, "Reconnected", true));
    let _ = sync(&mut ui, &center);
    assert_eq!(ui.list.len(), 1);
    assert_eq!(ui.list[0].title, "Reconnected");
}

#[test]
fn the_badge_counts_unread_and_shows_active_jobs() {
    let (_dir, center) = center();
    let mut ui = Notifications::default();
    center.post(note("a", Severity::Error, "One", false));
    center.post(note("b", Severity::Warning, "Two", false));
    center.post(job(JobStatus::Running, Some(0.2)));
    let _ = sync(&mut ui, &center);
    let badge = ui.badge();
    assert_eq!(badge.unread, 3);
    assert!(badge.active);

    // Opening the panel marks everything read.
    let effects = ui.update(Msg::Opened);
    apply(&center, &effects);
    let _ = sync(&mut ui, &center);
    assert_eq!(ui.badge().unread, 0);
    assert!(ui.badge().active, "the job is still running");
}

#[test]
fn toasts_show_once_and_expire_without_taking_focus() {
    let (_dir, center) = center();
    let mut ui = Notifications::default();
    center.post(note("a", Severity::Error, "Server unreachable", true));
    let effects = sync(&mut ui, &center);
    assert_eq!(ui.toasts().len(), 1);
    assert!(
        !effects.iter().any(|e| matches!(e, Effect::Focus(_))),
        "toasts never take focus"
    );
    let (id, stamp, after) = effects
        .iter()
        .find_map(|e| match e {
            Effect::ExpireToast { id, stamp, after } => Some((id.clone(), stamp.clone(), *after)),
            _ => None,
        })
        .expect("an expiry is scheduled");
    assert!(after >= Duration::from_secs(4));

    // The same post again (another list change) doesn't toast twice.
    let again = sync(&mut ui, &center);
    assert!(!again
        .iter()
        .any(|e| matches!(e, Effect::ExpireToast { .. })));

    let _ = ui.update(Msg::ToastExpired { id, stamp });
    assert!(ui.toasts().is_empty());
    assert_eq!(ui.list.len(), 1, "the entry stays in the centre");
}

#[test]
fn an_updated_entry_toasts_again_and_an_old_expiry_leaves_it() {
    let (_dir, center) = center();
    let mut ui = Notifications::default();
    center.post(note("a", Severity::Error, "Server unreachable", true));
    let first = sync(&mut ui, &center);
    let old = first
        .iter()
        .find_map(|e| match e {
            Effect::ExpireToast { id, stamp, .. } => Some((id.clone(), stamp.clone())),
            _ => None,
        })
        .expect("expiry");
    std::thread::sleep(Duration::from_millis(5));
    center.post(note("a", Severity::Info, "Reconnected", true));
    let second = sync(&mut ui, &center);
    assert!(second
        .iter()
        .any(|e| matches!(e, Effect::ExpireToast { .. })));
    let _ = ui.update(Msg::ToastExpired {
        id: old.0,
        stamp: old.1,
    });
    assert_eq!(ui.toasts().len(), 1, "the newer toast stays");
}

#[test]
fn entries_are_dismissed_one_or_all() {
    let (_dir, center) = center();
    let mut ui = Notifications::default();
    center.post(note("a", Severity::Info, "One", false));
    center.post(note("b", Severity::Info, "Two", false));
    let _ = sync(&mut ui, &center);
    let id = ui.list[0].id.clone();
    let effects = ui.update(Msg::Dismiss(id));
    apply(&center, &effects);
    let _ = sync(&mut ui, &center);
    assert_eq!(ui.list.len(), 1);
    let effects = ui.update(Msg::DismissAll);
    apply(&center, &effects);
    let _ = sync(&mut ui, &center);
    assert!(ui.list.is_empty());
}

#[test]
fn a_running_job_shows_progress_and_finishes() {
    let (_dir, center) = center();
    let mut ui = Notifications::default();
    center.post(job(JobStatus::Running, Some(0.4)));
    let _ = sync(&mut ui, &center);
    let progress = ui.list[0].job.as_ref().and_then(|j| j.progress);
    assert_eq!(progress, Some(0.4));
    assert!(ui.badge().active);
    center.post(job(JobStatus::Finished, Some(1.0)));
    let _ = sync(&mut ui, &center);
    assert_eq!(ui.list.len(), 1);
    assert!(!ui.badge().active);
}

#[test]
fn only_this_servers_and_general_entries_show() {
    let (_dir, center) = center();
    let mut ui = Notifications::default();
    let here = uuid::Uuid::new_v4();
    ui.profile = Some(here);
    center.post(NewNotification {
        profile_id: Some(uuid::Uuid::new_v4()),
        ..note("other", Severity::Error, "Another server", false)
    });
    center.post(NewNotification {
        profile_id: Some(here),
        ..note("mine", Severity::Error, "This server", false)
    });
    center.post(note("general", Severity::Info, "General", false));
    let _ = sync(&mut ui, &center);
    let titles: Vec<&str> = ui.list.iter().map(|n| n.title.as_str()).collect();
    assert_eq!(titles, vec!["General", "This server"]);
}

#[test]
fn the_bell_is_in_the_bar_and_opens_the_centre() {
    use semantic_stash_viewer_native::session::{Msg as SessionMsg, Session};
    use semantic_stash_viewer_native::shell::overlays::Overlay;
    let profile = stash_core::profiles::ServerProfile {
        id: uuid::Uuid::new_v4(),
        display_name: "Testing".into(),
        base_url: "http://localhost:9998/".parse().expect("url"),
        strict_tls: false,
        api_key: None,
        created_at: chrono::Utc::now(),
        last_used_at: None,
    };
    let (_dir, center) = center();
    center.post(note("a", Severity::Error, "Server unreachable", false));
    let (mut session, _) = Session::enter(profile, false);
    let _ = session.update(SessionMsg::Notifications(Msg::Changed(center.list())));
    let messages: Vec<SessionMsg> = {
        let mut ui = iced_test::simulator(session.view(None, None));
        // The unread count is on the bell.
        ui.click("1").expect("the bell's badge is in the bar");
        ui.into_messages().collect()
    };
    for m in messages {
        let _ = session.update(m);
    }
    assert_eq!(session.overlay, Overlay::Notifications);
}

#[test]
fn the_centre_closes_on_a_click_outside_or_its_close_button() {
    use semantic_stash_viewer_native::session::{Msg as SessionMsg, Session};
    use semantic_stash_viewer_native::shell::overlays::Overlay;
    use semantic_stash_viewer_native::shell::ShellMsg;
    let profile = stash_core::profiles::ServerProfile {
        id: uuid::Uuid::new_v4(),
        display_name: "Testing".into(),
        base_url: "http://localhost:9998/".parse().expect("url"),
        strict_tls: false,
        api_key: None,
        created_at: chrono::Utc::now(),
        last_used_at: None,
    };
    let (_dir, center) = center();
    center.post(note("a", Severity::Error, "Server unreachable", false));
    let (mut session, _) = Session::enter(profile, false);
    let _ = session.update(SessionMsg::Notifications(Msg::Changed(center.list())));
    let open = |s: &mut Session| {
        let _ = s.update(SessionMsg::Shell(ShellMsg::Notifications));
        assert_eq!(s.overlay, Overlay::Notifications);
    };
    let click = |s: &mut Session, at: Option<iced::Point>, label: Option<&str>| {
        let messages: Vec<SessionMsg> = {
            let mut ui = iced_test::simulator(s.view(None, None));
            match (at, label) {
                (Some(p), _) => {
                    ui.point_at(p);
                    let _ = ui.simulate(iced_test::simulator::click());
                }
                (None, Some(l)) => {
                    ui.click(l).expect("on screen");
                }
                _ => {}
            }
            ui.into_messages().collect()
        };
        for m in messages {
            let _ = s.update(m);
        }
    };

    open(&mut session);
    // Inside the panel: it stays.
    click(&mut session, None, Some("Server unreachable"));
    assert_eq!(session.overlay, Overlay::Notifications);
    // Outside it (the left of the window): it closes.
    click(&mut session, Some(iced::Point::new(40.0, 400.0)), None);
    assert_eq!(session.overlay, Overlay::None);

    open(&mut session);
    let _ = session.update(SessionMsg::Notifications(Msg::Close));
    assert_eq!(session.overlay, Overlay::None);
}
