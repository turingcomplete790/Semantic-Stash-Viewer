//! 008 T057–T058: play history rules (the web UI's player) and the on-disk queue
//! (contracts/play-history.md).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use stash_core::playback::history::{
    flush, resume_point, HistoryQueue, PlayHistoryReport, Viewing, CHECKPOINT_SECONDS,
};
use stash_core::AppError;

/// Play for `seconds` from `from`, one update a quarter-second apart (as the player sends them).
fn play(v: &mut Viewing, from: f64, seconds: f64, duration: f64) -> Vec<PlayHistoryReport> {
    let mut out = Vec::new();
    let steps = (seconds * 4.0).round() as usize;
    for i in 0..=steps {
        let t = from + i as f64 * 0.25;
        out.extend(v.observe(t, true, t, Some(duration)));
    }
    out
}

// ---- T057: rules ------------------------------------------------------------------------------

#[test]
fn a_checkpoint_follows_each_minute_of_playing_with_the_time_since_the_last() {
    let mut v = Viewing::start("7", None, true).expect("on");
    let reports = play(&mut v, 0.0, 150.0, 1000.0);
    assert_eq!(reports.len(), 2, "two full minutes");
    for r in &reports {
        assert!((r.play_duration - CHECKPOINT_SECONDS).abs() < 0.5, "{r:?}");
        assert!(!r.ended);
    }
    let last = v.end(150.0).expect("final");
    assert!(last.ended);
    assert!(
        (last.play_duration - 30.0).abs() < 0.5,
        "the remaining 30 s"
    );
    assert!((last.resume_time - 150.0).abs() < 0.5);
    assert!(v.end(151.0).is_none(), "one final report");
}

#[test]
fn paused_time_doesnt_count() {
    let mut v = Viewing::start("7", None, true).expect("on");
    play(&mut v, 0.0, 30.0, 1000.0);
    // Paused for ten minutes.
    for i in 0..=600 {
        let t = 30.0 + f64::from(i);
        assert!(v.observe(t, false, 30.0, Some(1000.0)).is_none());
    }
    let r = v.end(630.0).expect("final");
    assert!((r.play_duration - 30.0).abs() < 0.5, "{r:?}");
}

#[test]
fn a_resume_point_near_the_end_is_cleared() {
    assert_eq!(resume_point(980.0, 1000.0), 0.0, "98% counts as finished");
    assert_eq!(resume_point(979.0, 1000.0), 979.0);
    assert_eq!(resume_point(-1.0, 1000.0), 0.0);
    assert_eq!(
        resume_point(50.0, 0.0),
        50.0,
        "unknown duration keeps the point"
    );
}

#[test]
fn a_play_is_counted_once_on_the_first_report_past_the_minimum() {
    // Unset (0): the first report counts it.
    let mut v = Viewing::start("7", None, true).expect("on");
    let first = play(&mut v, 0.0, 61.0, 1000.0);
    assert!(first[0].add_play);
    let more = play(&mut v, 61.25, 70.0, 1000.0);
    assert!(more.iter().all(|r| !r.add_play), "once per viewing");
    assert!(!v.end(131.5).expect("final").add_play);

    // 20% of a 600 s file: not at the first minute (10%), at the second (20%).
    let mut v = Viewing::start("7", Some(20.0), true).expect("on");
    let reports = play(&mut v, 0.0, 121.0, 600.0);
    assert_eq!(
        reports.iter().map(|r| r.add_play).collect::<Vec<_>>(),
        vec![false, true]
    );

    // Short viewing below the minimum: no play.
    let mut v = Viewing::start("7", Some(50.0), true).expect("on");
    play(&mut v, 0.0, 10.0, 600.0);
    assert!(!v.end(10.0).expect("final").add_play);
}

#[test]
fn nothing_is_recorded_with_history_off_or_nothing_played() {
    assert!(Viewing::start("7", None, false).is_none());
    let mut v = Viewing::start("7", None, true).expect("on");
    assert!(v.observe(0.0, false, 0.0, Some(100.0)).is_none());
    assert!(v.end(5.0).is_none(), "opened and closed without playing");
}

#[test]
fn a_stalled_update_gap_isnt_counted_as_watching() {
    let mut v = Viewing::start("7", None, true).expect("on");
    v.observe(0.0, true, 0.0, Some(10_000.0));
    // The machine slept for an hour while "playing".
    v.observe(3600.0, true, 1.0, Some(10_000.0));
    assert!(v.played() <= 5.0, "{}", v.played());
}

// ---- T058: the queue --------------------------------------------------------------------------

fn report(scene: &str, resume: f64, duration: f64, add_play: bool) -> PlayHistoryReport {
    PlayHistoryReport {
        scene_id: scene.into(),
        resume_time: resume,
        play_duration: duration,
        add_play,
        ended: false,
        recorded_at: "2026-10-08T00:00:00Z".into(),
    }
}

#[test]
fn reports_are_kept_until_confirmed_and_survive_a_restart() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("play-history.json");
    let mut q = HistoryQueue::open(&path);
    q.push("p1", report("7", 60.0, 60.0, true)).expect("push");
    q.push("p1", report("8", 10.0, 10.0, false)).expect("push");
    // Written before sending: a fresh open sees both, in order.
    let reopened = HistoryQueue::open(&path);
    let ids: Vec<&str> = reopened
        .pending("p1")
        .iter()
        .map(|r| r.scene_id.as_str())
        .collect();
    assert_eq!(ids, vec!["7", "8"]);

    let first = q.next("p1").expect("first");
    assert_eq!(first.scene_id, "7");
    assert!(q.next("p1").is_none(), "one in flight at a time");
    q.confirm("p1").expect("confirm");
    assert_eq!(HistoryQueue::open(&path).pending("p1").len(), 1);

    let text = std::fs::read_to_string(&path).expect("file");
    let json: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(json["version"], 1);
    assert!(
        json["pending"]["p1"][0].get("ended").is_none(),
        "contract fields only"
    );
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .expect("dir")
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "atomic writes leave no temp files");
}

#[test]
fn waiting_reports_for_one_scene_merge_and_profiles_stay_apart() {
    let dir = tempfile::tempdir().expect("dir");
    let mut q = HistoryQueue::open(dir.path().join("q.json"));
    q.push("p1", report("7", 60.0, 60.0, true)).expect("push");
    q.push("p1", report("7", 90.0, 30.0, false)).expect("push");
    q.push("p2", report("7", 5.0, 5.0, false)).expect("push");
    let p1 = q.pending("p1");
    assert_eq!(p1.len(), 1);
    assert_eq!(p1[0].play_duration, 90.0, "durations add");
    assert_eq!(p1[0].resume_time, 90.0, "the latest resume point wins");
    assert!(p1[0].add_play, "a counted play is kept");
    assert_eq!(q.pending("p2").len(), 1, "per profile");

    // A report in flight isn't merged into: the new one waits behind it.
    let _ = q.next("p1");
    q.push("p1", report("7", 120.0, 30.0, false)).expect("push");
    assert_eq!(q.pending("p1").len(), 2);
    q.release("p1");
    assert_eq!(q.next("p1").expect("still first").play_duration, 90.0);
}

#[tokio::test]
async fn a_flush_sends_in_order_and_stops_at_a_failure() {
    let dir = tempfile::tempdir().expect("dir");
    let q = Mutex::new(HistoryQueue::open(dir.path().join("q.json")));
    for s in ["1", "2", "3"] {
        q.lock()
            .expect("lock")
            .push("p", report(s, 1.0, 1.0, false))
            .expect("push");
    }
    let order = Mutex::new(Vec::new());
    let outcome = flush(&q, "p", None, |r| {
        order.lock().expect("lock").push(r.scene_id.clone());
        let fail = r.scene_id == "2";
        async move {
            if fail {
                Err(AppError::NotConnected)
            } else {
                Ok(())
            }
        }
    })
    .await;
    assert_eq!(
        *order.lock().expect("lock"),
        vec!["1", "2"],
        "in order, stopping at the failure"
    );
    assert_eq!(outcome.sent, 1);
    assert!(outcome.failed.is_some());
    assert_eq!(outcome.left, 2, "the failed one and the rest stay");
    assert_eq!(q.lock().expect("lock").pending("p")[0].scene_id, "2");
}

#[tokio::test]
async fn the_flush_deadline_leaves_the_rest_queued() {
    let dir = tempfile::tempdir().expect("dir");
    let q = Mutex::new(HistoryQueue::open(dir.path().join("q.json")));
    for s in ["1", "2", "3"] {
        q.lock()
            .expect("lock")
            .push("p", report(s, 1.0, 1.0, false))
            .expect("push");
    }
    let deadline = Instant::now() + Duration::from_millis(150);
    let outcome = flush(&q, "p", Some(deadline), |_| async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok::<_, AppError>(())
    })
    .await;
    assert!(
        Instant::now() < deadline + Duration::from_millis(100),
        "kept to the deadline"
    );
    assert!(outcome.sent < 3);
    assert_eq!(outcome.left, 3 - outcome.sent);
}
