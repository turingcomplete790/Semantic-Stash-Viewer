//! The UI bench (007 T040; contracts/measurements.md; `SSV_DEBUG_BENCH=1`): section navigation,
//! a control press, tab switches with 20 tabs, and the Scenes rows (page change, page jump, and
//! scrolling a 1000-card page cold and cached in each mode), in the real app, as `MEASURE` lines
//! with the web bench's names and shapes.
//!
//! It's a small script run one step per window frame: each step sends real messages through the
//! session (so `update`, `view`, and the effects all count) and waits on frames, conditions, or
//! time. "Drawn" means two frames after the change, as the web bench's two animation frames.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::effects::Effect;
use crate::machine::Step;
use crate::screens::scenes::keyboard::{GridAction, PageDirection};
use crate::screens::scenes::thumbs::thumb_key;
use crate::screens::scenes::{ScenesMsg, Thumbs};
use crate::screens::{Mode, Screen, Section};
use crate::session::{self, Session};
use crate::shell::overlays;
use crate::shell::ShellMsg;
use crate::widgets::scroll_watch::{AutoScroll, AutoScrolled};
use Act as A;

/// The scroll speed: the web bench's 64 px per frame at 60 Hz, as a speed, so displays with other
/// refresh rates scroll the same distance per second (a 144 Hz display moves 26.7 px a frame).
pub const SCROLL_SPEED: f32 = 64.0 * 60.0;

/// A message for the session.
#[derive(Debug, Clone)]
enum Act {
    Shell(ShellMsg),
    CloseOverlay,
    /// Forget the thumbnails in memory (they come back from the disk cache): a cold scroll.
    ClearThumbs,
    /// Go to a pseudo-random page.
    Jump,
    /// Close every tab the bench opened.
    CloseBenchTabs,
    /// Open a scene from the active Scenes page (the first two minutes or longer) in this tab and
    /// play it, muted.
    PlayFirst,
    StopPlayback,
    /// Select a tab that isn't the playing one.
    SelectOther,
    /// The now-playing bar's "Back to scene".
    BackToScene,
    /// Select the playing tab (as a tab switch).
    SelectOwner,
}

#[derive(Debug, Clone, Copy)]
enum Cond {
    /// The active Scenes page is showing.
    Ready,
    /// A different page is showing, and its visible thumbnails are here.
    PageShown,
    /// A different page's cards are showing.
    CardsShown,
    /// At least this many cards are showing.
    Cards(usize),
    /// Every card on the page has its thumbnail.
    AllThumbs,
    /// The player is playing.
    Playing,
}

#[derive(Debug, Clone)]
enum Op {
    Act(Act),
    Frames(u32),
    Until(Cond, Duration),
    Sleep(Duration),
    /// Note the active page's first card (for `PageShown`, `CardsShown`).
    Remember,
    Start,
    /// Add `now - start` to a sample set, if the last wait succeeded.
    Sample(&'static str),
    /// Print a sample set's summary.
    Report(&'static str),
    /// Scroll the active Scenes page at [`SCROLL_SPEED`] for this long (inside the scrollable:
    /// no messages per frame, as when a person scrolls).
    Scroll(Duration),
    /// Print the scroll just done as this line.
    ScrollReport(&'static str),
    /// Print how long the page's thumbnails took (since `Start`) and whether they all came.
    ReportThumbs,
    /// Print the 1000-card page's size (the Testing library may hold fewer scenes).
    ReportCards,
    /// Ask the harness to sample the process tree's memory and CPU for this long (the app holds
    /// still meanwhile).
    ProcSample(&'static str, u64),
    /// Note the frames shown so far (before switching tabs).
    VideoMark,
    /// If the playing tab is showing, wait (a few frames) for a new video frame on screen.
    VideoCheck,
    Done,
}

/// Where a step is waiting.
#[derive(Debug, Clone, Copy)]
enum Wait {
    None,
    Frames(u32),
    Until {
        cond: Cond,
        deadline: Instant,
    },
    Sleep(Instant),
    /// The scrollable is scrolling itself; its timings arrive as a message.
    Scrolling,
    /// Waiting this many more frames for a video frame to be shown.
    Video(u32),
    /// Idle until this time: no frame clock (the app redraws only if something changes), so
    /// memory and CPU samples see the app at rest. A timer wakes the bench.
    Idle(Instant),
}

pub struct Bench {
    ops: Vec<Op>,
    at: usize,
    wait: Wait,
    started: Instant,
    last_ok: bool,
    samples: HashMap<&'static str, Vec<f64>>,
    remembered: Option<String>,
    last_frame: Option<Instant>,
    scrolled: Option<AutoScrolled>,
    scroll_id: u64,
    seed: u32,
    /// Tabs open before the bench, so it can close its own.
    first_bench_tab: usize,
    /// Frames shown when the last `VideoMark` ran.
    video_mark: u64,
    /// Switches to the playing tab, and how many showed a new video frame.
    video_checked: u32,
    video_sized: u32,
}

fn script() -> Vec<Op> {
    use Op::*;
    let shell = |m: ShellMsg| Op::Act(A::Shell(m));
    let ms = Duration::from_millis;
    // `SSV_BENCH_ONLY=scroll`: only the 1000-card scroll (for profiling it).
    let only_scroll = super::var("SSV_BENCH_ONLY").as_deref() == Some("scroll");
    let mut ops = vec![
        Frames(10),
        // Section navigation in one new tab: Home ↔ Scenes, each until drawn.
        shell(ShellMsg::NewTab),
        Frames(2),
        shell(ShellMsg::Section(Section::Scenes)),
        Until(Cond::Ready, ms(15_000)),
        Frames(2),
        shell(ShellMsg::Section(Section::Home)),
        Frames(2),
    ];
    for i in 0..20 {
        let section = if i % 2 == 0 {
            Section::Scenes
        } else {
            Section::Home
        };
        ops.extend([
            Start,
            shell(ShellMsg::Section(section)),
            Frames(2),
            Sample("nav"),
        ]);
    }
    ops.push(Report("nav"));

    // A control press: keyboard help opens.
    for _ in 0..10 {
        ops.extend([
            Start,
            shell(ShellMsg::KeyboardHelp),
            Frames(2),
            Sample("press"),
            Op::Act(A::CloseOverlay),
            Frames(2),
        ]);
    }
    ops.push(Report("press"));

    // A scene playing in this tab (muted), then 20 tabs open and a fixed pseudo-random walk
    // over them.
    ops.extend([
        shell(ShellMsg::Section(Section::Scenes)),
        Until(Cond::Ready, ms(15_000)),
        Op::Act(A::PlayFirst),
        Until(Cond::Playing, ms(20_000)),
        Frames(30),
    ]);
    let kinds = [Section::Scenes, Section::Home, Section::Settings];
    for i in 0..19 {
        ops.extend([
            shell(ShellMsg::NewTab),
            shell(ShellMsg::Section(kinds[i % 3])),
        ]);
    }
    ops.push(Frames(4));
    let mut seed = 7usize;
    for i in 0..40 {
        seed = (seed * 13 + 5) % 20;
        // Every fifth switch goes to the playing tab (the walk alone may never land there).
        let switch = if i % 5 == 4 {
            Op::Act(A::SelectOwner)
        } else {
            shell(ShellMsg::SelectIndex(seed))
        };
        ops.extend([
            VideoMark,
            Start,
            switch,
            Frames(2),
            Sample("tabs"),
            VideoCheck,
        ]);
    }
    ops.push(Report("tabs"));
    // The now-playing bar appears on leaving the playing tab; "Back to scene" returns to it.
    ops.extend([Op::Act(A::BackToScene), Frames(4)]);
    for _ in 0..10 {
        ops.extend([
            Start,
            Op::Act(A::SelectOther),
            Frames(2),
            Sample("now-playing"),
            Start,
            Op::Act(A::BackToScene),
            Frames(2),
            Sample("back"),
        ]);
    }
    ops.extend([
        Report("now-playing"),
        Report("back"),
        Op::Act(A::StopPlayback),
        Op::Act(A::CloseBenchTabs),
        Frames(2),
    ]);
    if only_scroll {
        ops.clear();
        ops.push(Frames(10));
    }

    // Scenes: a fresh tab at 50 per page.
    ops.extend([
        shell(ShellMsg::NewTab),
        shell(ShellMsg::Section(Section::Scenes)),
        Until(Cond::Ready, ms(15_000)),
        Frames(2),
    ]);
    for i in 0..20 {
        let dir = if i < 10 {
            PageDirection::Next
        } else {
            PageDirection::Previous
        };
        ops.extend([
            // A glance at the page first, as a person would; meanwhile the next is prefetched.
            Sleep(ms(1000)),
            Remember,
            Start,
            shell(ShellMsg::Scenes(ScenesMsg::Grid(GridAction::Page(dir)))),
            Until(Cond::PageShown, ms(5000)),
            Frames(2),
            Sample("page-change"),
        ]);
    }
    ops.push(Report("page-change"));
    for _ in 0..5 {
        ops.extend([
            Remember,
            Start,
            Op::Act(A::Jump),
            Until(Cond::CardsShown, ms(5000)),
            Frames(2),
            Sample("page-jump"),
        ]);
    }
    ops.push(Report("page-jump"));
    if only_scroll {
        ops.truncate(1);
        ops.extend([
            shell(ShellMsg::NewTab),
            shell(ShellMsg::Section(Section::Scenes)),
            Until(Cond::Ready, ms(15_000)),
        ]);
    }

    // A 1000-card page, scrolled in each mode: cold (thumbnails arriving), then cached.
    ops.extend([
        shell(ShellMsg::Scenes(ScenesMsg::Page(1))),
        Until(Cond::Ready, ms(15_000)),
        shell(ShellMsg::Scenes(ScenesMsg::PageSize(1000))),
        Until(Cond::Cards(1000), ms(15_000)),
        ReportCards,
    ]);
    for (mode, cold, warm) in [
        (
            Mode::Grid,
            "scenes-scroll-1000-grid-cold",
            "scenes-scroll-1000-grid",
        ),
        (
            Mode::List,
            "scenes-scroll-1000-list-cold",
            "scenes-scroll-1000-list",
        ),
    ] {
        ops.extend([
            shell(ShellMsg::Scenes(ScenesMsg::Mode(mode))),
            Op::Act(A::ClearThumbs),
            shell(ShellMsg::Scenes(ScenesMsg::Scrolled { y: 0.0 })),
            Frames(2),
            Scroll(ms(5000)),
            ScrollReport(cold),
            Start,
            Until(Cond::AllThumbs, ms(60_000)),
            ReportThumbs,
            Scroll(ms(5000)),
            ScrollReport(warm),
        ]);
    }
    ops.extend([
        shell(ShellMsg::Scenes(ScenesMsg::Mode(Mode::Grid))),
        shell(ShellMsg::Scenes(ScenesMsg::PageSize(50))),
        Until(Cond::Ready, ms(15_000)),
        // At rest on a 50-card page: memory and CPU (007 T057).
        Sleep(ms(5000)),
        ProcSample("idle-scenes-50", 5),
        Op::Act(A::CloseBenchTabs),
        Frames(2),
        Done,
    ]);
    ops
}

fn round(x: f64, digits: i32) -> f64 {
    let s = 10f64.powi(digits);
    (x * s).round() / s
}

fn report(name: &str, mut value: serde_json::Value) {
    value["bench"] = serde_json::Value::from(name);
    super::emit(&value);
}

impl Bench {
    pub fn new(session: &mut Session) -> Self {
        session.saving_suspended = true;
        Self {
            ops: script(),
            at: 0,
            wait: Wait::None,
            started: Instant::now(),
            last_ok: true,
            samples: HashMap::new(),
            remembered: None,
            last_frame: None,
            scrolled: None,
            scroll_id: 0,
            seed: 11,
            first_bench_tab: session.shell.tabs.len(),
            video_mark: 0,
            video_checked: 0,
            video_sized: 0,
        }
    }

    fn scenes(session: &Session) -> Option<&crate::screens::scenes::ScenesState> {
        match session.shell.active().current() {
            Screen::Scenes(s) => Some(s),
            _ => None,
        }
    }

    fn first_card(session: &Session) -> Option<String> {
        Self::scenes(session)
            .filter(|s| s.ready())
            .and_then(|s| s.cards().first().map(|c| c.id.clone()))
    }

    fn holds(&self, cond: Cond, session: &Session) -> bool {
        if let Cond::Playing = cond {
            return session.playback.snapshot().state == player::PlayerStateKind::Playing;
        }
        let Some(s) = Self::scenes(session) else {
            return false;
        };
        if !s.ready() {
            return false;
        }
        let changed = || Self::first_card(session) != self.remembered;
        match cond {
            Cond::Ready => true,
            Cond::Playing => unreachable!("checked before the Scenes screen"),
            Cond::Cards(n) => s.cards().len() >= n.min(s.count().unwrap_or(0) as usize),
            Cond::CardsShown => changed(),
            Cond::AllThumbs => s
                .cards()
                .iter()
                .filter_map(thumb_key)
                .all(|k| session.thumbs.get(&k).is_some()),
            Cond::PageShown => {
                let layout = session.shell.layout;
                let rows = (layout.viewport() / layout.row_height(s.mode)).ceil() as usize + 1;
                let visible = rows * layout.cols(s.mode);
                changed()
                    && s.cards()
                        .iter()
                        .take(visible)
                        .filter_map(thumb_key)
                        .all(|k| session.thumbs.get(&k).is_some())
            }
        }
    }

    fn act(&mut self, act: Act, session: &mut Session) -> Vec<Effect> {
        let step = match act {
            Act::Shell(m) => session.update(session::Msg::Shell(m)),
            Act::CloseOverlay => session.update(session::Msg::Overlay(overlays::Msg::Close)),
            Act::ClearThumbs => {
                session.thumbs = Thumbs::default();
                Step::none()
            }
            Act::Jump => {
                let last = Self::scenes(session)
                    .and_then(|s| s.last_page())
                    .unwrap_or(1);
                self.seed = (self.seed * 7919 + 13) % 10007;
                let target = 1 + self.seed % last.max(1);
                session.update(session::Msg::Shell(ShellMsg::Scenes(ScenesMsg::Page(
                    target,
                ))))
            }
            Act::PlayFirst => {
                // One long enough to keep playing through the tab rows.
                let Some(card) = Self::scenes(session).and_then(|s| {
                    s.cards()
                        .iter()
                        .find(|c| c.duration_seconds.unwrap_or(0.0) >= 120.0)
                        .or_else(|| s.cards().first())
                        .cloned()
                }) else {
                    return Vec::new();
                };
                let mut effects = session
                    .update(session::Msg::Shell(ShellMsg::Open(Screen::scene(
                        &card.id,
                        &card.title,
                    ))))
                    .effects;
                effects.extend(
                    session
                        .update(session::Msg::Shell(ShellMsg::Play(card.id)))
                        .effects,
                );
                effects.push(Effect::Player(crate::effects::PlayerAction::SetMuted(true)));
                return effects;
            }
            Act::StopPlayback => {
                session.update(session::Msg::Playback(session::playback::Msg::Close))
            }
            Act::SelectOther => {
                let owner = session.playback.owner;
                match session.shell.tabs.iter().find(|t| Some(t.id) != owner) {
                    Some(t) => {
                        let id = t.id;
                        session.update(session::Msg::Shell(ShellMsg::Select(id)))
                    }
                    None => Step::none(),
                }
            }
            Act::BackToScene => session.update(session::Msg::BackToScene),
            Act::SelectOwner => match session.playback.owner {
                Some(id) => session.update(session::Msg::Shell(ShellMsg::Select(id))),
                None => Step::none(),
            },
            Act::CloseBenchTabs => {
                let mut effects = Vec::new();
                while session.shell.tabs.len() > self.first_bench_tab.max(1) {
                    let id = session.shell.tabs[session.shell.tabs.len() - 1].id;
                    effects.extend(
                        session
                            .update(session::Msg::Shell(ShellMsg::Close(id)))
                            .effects,
                    );
                }
                return effects;
            }
        };
        step.effects
    }

    /// One frame: advance the script as far as it can go. Returns the effects to run and whether
    /// the bench has finished.
    pub fn frame(&mut self, now: Instant, session: &mut Session) -> (Vec<Effect>, bool) {
        self.last_frame = Some(now);
        let mut effects = Vec::new();

        // Finish the current wait, if it's over.
        match self.wait {
            Wait::None => {}
            Wait::Frames(n) => {
                if n > 1 {
                    self.wait = Wait::Frames(n - 1);
                    return (effects, false);
                }
                self.wait = Wait::None;
            }
            Wait::Until { cond, deadline } => {
                if self.holds(cond, session) {
                    self.last_ok = true;
                    self.wait = Wait::None;
                } else if now >= deadline {
                    self.last_ok = false;
                    self.wait = Wait::None;
                } else {
                    return (effects, false);
                }
            }
            Wait::Sleep(until) => {
                if now < until {
                    return (effects, false);
                }
                self.wait = Wait::None;
            }
            Wait::Idle(until) => {
                if now < until {
                    return (effects, false);
                }
                self.wait = Wait::None;
            }
            Wait::Video(left) => {
                if crate::player::video::frame_counts().1 > self.video_mark {
                    self.video_sized += 1;
                    self.wait = Wait::None;
                } else if left > 1 {
                    self.wait = Wait::Video(left - 1);
                    return (effects, false);
                } else {
                    self.wait = Wait::None;
                }
            }
            Wait::Scrolling => {
                if self.scrolled.is_none() {
                    return (effects, false);
                }
                self.wait = Wait::None;
            }
        }

        // Run steps until one waits.
        while self.at < self.ops.len() {
            let op = self.ops[self.at].clone();
            self.at += 1;
            match op {
                Op::Act(a) => effects.extend(self.act(a, session)),
                Op::Frames(n) => {
                    self.wait = Wait::Frames(n);
                    return (effects, false);
                }
                Op::Until(cond, timeout) => {
                    if self.holds(cond, session) {
                        self.last_ok = true;
                    } else {
                        self.wait = Wait::Until {
                            cond,
                            deadline: now + timeout,
                        };
                        return (effects, false);
                    }
                }
                Op::Sleep(d) => {
                    self.wait = Wait::Sleep(now + d);
                    return (effects, false);
                }
                Op::Remember => self.remembered = Self::first_card(session),
                Op::Start => {
                    self.started = Instant::now();
                    self.last_ok = true;
                }
                Op::Sample(name) => {
                    if self.last_ok {
                        let ms = self.started.elapsed().as_secs_f64() * 1000.0;
                        self.samples.entry(name).or_default().push(ms);
                    }
                }
                Op::Report(name) => {
                    let line = match name {
                        "nav" => "nav-first-paint",
                        "press" => "control-press",
                        "tabs" => "tab-switch-20-tabs",
                        "now-playing" => "now-playing-appears",
                        "back" => "back-to-scene",
                        "page-change" => "scenes-page-change",
                        "page-jump" => "scenes-page-jump",
                        other => other,
                    };
                    let samples = self.samples.remove(name).unwrap_or_default();
                    let mut value = super::summary(&samples, 0);
                    let playing = session.playback.active();
                    match name {
                        "tabs" => {
                            value["playing"] = playing.into();
                            value["videoSized"] =
                                format!("{}/{}", self.video_sized, self.video_checked).into();
                        }
                        "now-playing" => value["playing"] = playing.into(),
                        _ => {}
                    }
                    report(line, value);
                }
                Op::Scroll(duration) => {
                    self.scroll_id += 1;
                    self.scrolled = None;
                    let auto = AutoScroll {
                        id: self.scroll_id,
                        px_per_second: SCROLL_SPEED,
                        duration,
                    };
                    effects.extend(
                        session
                            .update(session::Msg::Shell(ShellMsg::Scenes(
                                ScenesMsg::AutoScroll(Some(auto)),
                            )))
                            .effects,
                    );
                    self.wait = Wait::Scrolling;
                    return (effects, false);
                }
                Op::ScrollReport(name) => {
                    let line = self.scroll_report();
                    report(name, line);
                }
                Op::ReportThumbs => {
                    let mode = Self::scenes(session).map_or("grid", |s| match s.mode {
                        Mode::Grid => "grid",
                        Mode::List => "list",
                    });
                    report(
                        &format!("scenes-thumbs-1000-{mode}"),
                        serde_json::json!({
                            "loaded": self.last_ok,
                            "ms": (self.started.elapsed().as_secs_f64() * 1000.0).round(),
                        }),
                    );
                }
                Op::VideoMark => {
                    self.video_mark = crate::player::video::frame_counts().1;
                }
                Op::VideoCheck => {
                    if session.playback.owner == Some(session.shell.active().id) {
                        self.video_checked += 1;
                        // A 24–30 fps video shows a new frame within about 40 ms.
                        self.wait = Wait::Video(12);
                        return (effects, false);
                    }
                }
                Op::ProcSample(name, secs) => {
                    super::emit(&serde_json::json!({"sample": name, "secs": secs}));
                    let rest = Duration::from_secs(secs + 1);
                    self.wait = Wait::Idle(now + rest);
                    effects.push(Effect::WakeBench(rest));
                    return (effects, false);
                }
                Op::ReportCards => {
                    let n = Self::scenes(session).map_or(0, |s| s.cards().len());
                    let mut line = serde_json::json!({"cards": n});
                    if n < 1000 {
                        line["note"] = "the library holds fewer than 1000 scenes".into();
                    }
                    report("scenes-page-1000", line);
                }
                Op::Done => {
                    session.saving_suspended = false;
                    report("done", serde_json::json!({}));
                    return (effects, true);
                }
            }
        }
        (effects, true)
    }

    /// Whether the bench needs frame messages now (not while the scrollable times itself).
    pub fn wants_frames(&self) -> bool {
        !matches!(self.wait, Wait::Scrolling | Wait::Idle(_))
    }

    /// The scrollable finished a bench scroll.
    pub fn scrolled(&mut self, result: AutoScrolled) {
        if result.id == self.scroll_id {
            self.scrolled = Some(result);
            // Frames resume and the script carries on.
            self.wait = Wait::None;
        }
    }

    /// The scroll line for the frames just recorded.
    fn scroll_report(&mut self) -> serde_json::Value {
        let Some(result) = self.scrolled.take() else {
            return serde_json::json!({"n": 0, "invalid": "the scroll didn't run"});
        };
        let mut base = result.baseline;
        base.sort_by(f64::total_cmp);
        let baseline = base.get(base.len() / 2).copied().unwrap_or(1000.0 / 60.0);
        // iced can deliver a second redraw event within one presented frame (after a message);
        // a sliver like that is folded into the frame it belongs to.
        let mut frames: Vec<f64> = Vec::new();
        let mut carry = 0.0;
        for f in result.frames.into_iter().skip(1) {
            if f < 2.0 {
                carry += f;
            } else {
                frames.push(f + carry);
                carry = 0.0;
            }
        }
        let missed = frames.iter().filter(|f| **f > baseline * 1.5).count();
        let elapsed: f64 = frames.iter().sum();
        let mut line = super::summary(&frames, 1);
        line["baselineMs"] = round(baseline, 1).into();
        line["pxPerSecond"] = f64::from(SCROLL_SPEED).into();
        line["fps"] = if elapsed > 0.0 {
            round(frames.len() as f64 / elapsed * 1000.0, 1).into()
        } else {
            serde_json::Value::Null
        };
        line["missedPercent"] = if frames.is_empty() {
            serde_json::Value::Null
        } else {
            round(missed as f64 / frames.len() as f64 * 100.0, 1).into()
        };
        line
    }
}
