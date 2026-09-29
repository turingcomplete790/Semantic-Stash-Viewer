//! The notification centre (004 US3; data-model "Notification", "NotificationsFile").

use stash_core::shell::notifications::{
    JobProgress, JobStatus, NewNotification, NotificationCenter, NotificationKind, Severity,
};
use stash_core::shell::MAX_NOTIFICATIONS;

fn note(key: Option<&str>, severity: Severity, title: &str) -> NewNotification {
    NewNotification {
        key: key.map(str::to_owned),
        profile_id: None,
        kind: NotificationKind::Connection,
        severity,
        title: title.into(),
        detail: None,
        toast: false,
        job: None,
    }
}

fn job(key: &str, status: JobStatus) -> NewNotification {
    NewNotification {
        kind: NotificationKind::Job,
        job: Some(JobProgress {
            status,
            progress: Some(0.5),
            started_at: None,
            ended_at: None,
        }),
        ..note(Some(key), Severity::Info, "Scanning...")
    }
}

fn center() -> (tempfile::TempDir, NotificationCenter) {
    let dir = tempfile::tempdir().expect("tempdir");
    let center = NotificationCenter::open(dir.path().join("shell/notifications.json"));
    (dir, center)
}

#[test]
fn posts_newest_first() {
    let (_dir, c) = center();
    c.post(note(None, Severity::Info, "first"));
    c.post(note(None, Severity::Info, "second"));
    let titles: Vec<_> = c.list().into_iter().map(|n| n.title).collect();
    assert_eq!(titles, ["second", "first"]);
    assert!(c.list().iter().all(|n| !n.read));
}

#[test]
fn a_keyed_post_updates_in_place_and_goes_unread_only_if_worse() {
    let (_dir, c) = center();
    let first = c.post(note(
        Some("connection:p"),
        Severity::Warning,
        "Server unreachable",
    ));
    c.mark_read(std::slice::from_ref(&first.id));
    let updated = c.post(note(Some("connection:p"), Severity::Info, "Reconnected"));
    assert_eq!(updated.id, first.id);
    assert_eq!(c.list().len(), 1);
    assert_eq!(c.list()[0].title, "Reconnected");
    assert!(c.list()[0].read, "less severe update stays read");
    let worse = c.post(note(
        Some("connection:p"),
        Severity::Error,
        "API key rejected",
    ));
    assert_eq!(worse.id, first.id);
    assert!(!c.list()[0].read, "more severe update is unread again");
}

#[test]
fn dismissing_removes_and_dismiss_all_keeps_active_jobs() {
    let (_dir, c) = center();
    let a = c.post(note(None, Severity::Info, "a"));
    c.post(note(None, Severity::Info, "b"));
    c.post(job("job:p:1", JobStatus::Running));
    c.post(job("job:p:2", JobStatus::Finished));
    c.dismiss(&a.id);
    assert_eq!(c.list().len(), 3);
    c.dismiss_all();
    let left = c.list();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].key.as_deref(), Some("job:p:1"));
}

#[test]
fn caps_at_200_and_never_drops_an_active_job() {
    let (_dir, c) = center();
    c.post(job("job:p:keep", JobStatus::Running));
    for i in 0..(MAX_NOTIFICATIONS + 20) {
        c.post(note(None, Severity::Info, &format!("n{i}")));
    }
    let list = c.list();
    assert_eq!(list.len(), MAX_NOTIFICATIONS);
    assert!(list.iter().any(|n| n.key.as_deref() == Some("job:p:keep")));
    assert_eq!(list[0].title, format!("n{}", MAX_NOTIFICATIONS + 19));
}

#[test]
fn persists_and_marks_active_jobs_unknown_on_reload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("notifications.json");
    {
        let c = NotificationCenter::open(&path);
        c.post(note(
            Some("connection:p"),
            Severity::Warning,
            "Server unreachable",
        ));
        c.post(job("job:p:1", JobStatus::Running));
        c.post(job("job:p:2", JobStatus::Finished));
    }
    let c = NotificationCenter::open(&path);
    let list = c.list();
    assert_eq!(list.len(), 3);
    let status = |key: &str| {
        list.iter()
            .find(|n| n.key.as_deref() == Some(key))
            .and_then(|n| n.job.as_ref())
            .map(|j| j.status)
    };
    assert_eq!(status("job:p:1"), Some(JobStatus::Unknown));
    assert_eq!(status("job:p:2"), Some(JobStatus::Finished));
    // Toasts are for live changes only, not for what was there at launch.
    assert!(list.iter().all(|n| !n.toast));
}

#[test]
fn broadcasts_the_full_list_on_every_change() {
    let (_dir, c) = center();
    let mut rx = c.subscribe();
    c.post(note(None, Severity::Info, "hello"));
    assert!(rx.has_changed().expect("open"));
    assert_eq!(rx.borrow_and_update().len(), 1);
    let id = c.list()[0].id.clone();
    c.mark_read(&[id]);
    assert!(rx.has_changed().expect("open"));
    assert!(rx.borrow_and_update()[0].read);
}
