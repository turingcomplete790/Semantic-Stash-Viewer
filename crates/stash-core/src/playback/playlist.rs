//! The queue's playback rules as data and pure functions (011 research R1): settings, the
//! playing position, and an event go in; the next state and one command come out. No clock,
//! randomness, I/O, or UI: a shuffle seed arrives with the event that starts a round.
//!
//! Positions are indices into the whole result (`0..count`, research R2); the UI maps an index to
//! a page and a card.

use serde::{Deserialize, Serialize};

/// One Repeat control, cycled Off → All → One (research R4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

impl Repeat {
    pub fn next(self) -> Repeat {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// The tab's saved playlist controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub autoplay: bool,
    pub repeat: Repeat,
    pub shuffle: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            autoplay: true,
            repeat: Repeat::Off,
            shuffle: false,
        }
    }
}

/// A shuffled round: `first`, then the seeded permutation of `0..count` without `first`; `pos` is
/// the playing scene's place in that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Round {
    pub seed: u64,
    pub first: u32,
    pub pos: u32,
}

/// The machine's state for one queue: a small value, copied rather than mutated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Playlist {
    pub settings: Settings,
    /// Scenes in the result (0: empty).
    pub count: u32,
    /// Result index of the playing (or last played) scene: where Next and Previous move from.
    pub playing: Option<u32>,
    /// Whether that scene is in the player now (choosing it again doesn't restart it).
    pub live: bool,
    /// Present while Shuffle is on and the result isn't empty.
    pub round: Option<Round>,
    /// Deleted scenes skipped in a row; reset when a scene starts.
    pub skips: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The playing scene reached its end.
    Ended,
    /// The playing scene couldn't be played.
    Failed,
    /// The scene to play no longer exists in Stash.
    Skipped,
    Next,
    Previous,
    /// The viewer chose this scene (a click, the keyboard's rest, Enter).
    Chosen(u32),
    /// The queue may start on its own at this scene (choosing Queue, new results, a tab switch).
    Autostart(u32),
    ResultsChanged {
        count: u32,
        seed: u64,
    },
    SetAutoplay(bool),
    CycleRepeat,
    ToggleShuffle {
        seed: u64,
    },
    /// A scene really began playing.
    Started(u32),
    /// Nothing plays in this queue (the player closed, or another tab took it); `Some(i)`: the
    /// queue continues from scene `i` (the selection).
    Idle(Option<u32>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Play scene `i` of the result.
    Play(u32),
    /// Play the playing scene again from the beginning.
    Replay,
    Stop,
    Nothing,
}

impl Playlist {
    pub fn new(settings: Settings, count: u32) -> Playlist {
        Playlist {
            settings,
            count,
            ..Playlist::default()
        }
    }
}

/// The machine: one `match` on the event, a new value out (the input is untouched).
pub fn step(state: &Playlist, event: Event) -> (Playlist, Command) {
    let s = *state;
    match event {
        Event::ResultsChanged { count, seed } => {
            let round = (s.settings.shuffle && count > 0).then(|| fresh_round(seed, count, None));
            let next = Playlist {
                count,
                playing: None,
                live: false,
                round,
                skips: 0,
                ..s
            };
            (next, Command::Nothing)
        }
        Event::SetAutoplay(on) => {
            let settings = Settings {
                autoplay: on,
                ..s.settings
            };
            (Playlist { settings, ..s }, Command::Nothing)
        }
        Event::CycleRepeat => {
            let settings = Settings {
                repeat: s.settings.repeat.next(),
                ..s.settings
            };
            (Playlist { settings, ..s }, Command::Nothing)
        }
        Event::ToggleShuffle { seed } => {
            let shuffle = !s.settings.shuffle;
            let round = (shuffle && s.count > 0).then(|| match s.playing {
                Some(p) => Round {
                    seed,
                    first: p,
                    pos: 0,
                },
                None => fresh_round(seed, s.count, None),
            });
            let settings = Settings {
                shuffle,
                ..s.settings
            };
            (
                Playlist {
                    settings,
                    round,
                    ..s
                },
                Command::Nothing,
            )
        }
        _ if s.count == 0 => {
            let cmd = match event {
                Event::Ended | Event::Failed | Event::Skipped => Command::Stop,
                _ => Command::Nothing,
            };
            (s, cmd)
        }
        Event::Failed => (s, Command::Stop),
        Event::Idle(at) => {
            let playing = at.filter(|&i| i < s.count).or(s.playing);
            let round = match (s.round, playing) {
                (Some(r), Some(i)) => Some(follow(r, s.count, i)),
                (r, _) => r,
            };
            let next = Playlist {
                playing,
                live: false,
                round,
                ..s
            };
            (next, Command::Nothing)
        }
        Event::Started(i) if i < s.count => {
            let round = s.round.map(|r| follow(r, s.count, i));
            let next = Playlist {
                playing: Some(i),
                live: true,
                round,
                skips: 0,
                ..s
            };
            (next, Command::Nothing)
        }
        Event::Started(_) => (s, Command::Nothing),
        Event::Chosen(i) => choose(s, i),
        Event::Autostart(i) if s.settings.autoplay => choose(s, i),
        Event::Autostart(_) => (s, Command::Nothing),
        Event::Ended => match (s.playing, s.settings) {
            (None, _) => (s, Command::Stop),
            (
                Some(_),
                Settings {
                    repeat: Repeat::One,
                    ..
                },
            ) => (s, Command::Replay),
            (
                Some(_),
                Settings {
                    autoplay: false, ..
                },
            ) => (s, Command::Stop),
            (Some(_), _) => forward(s),
        },
        Event::Next => forward(s),
        Event::Skipped if s.skips + 1 >= s.count => (
            Playlist {
                skips: s.count,
                ..s
            },
            Command::Stop,
        ),
        Event::Skipped => {
            let skips = s.skips + 1;
            forward(Playlist { skips, ..s })
        }
        Event::Previous => backward(s),
    }
}

/// The round, now at scene `i`: unchanged when `i` is its current scene; otherwise a round from
/// `i` (a scene played from elsewhere), so every scene still plays once per round.
fn follow(r: Round, count: u32, i: u32) -> Round {
    if order(&r, count, r.pos) == i {
        r
    } else {
        Round {
            seed: mix(r.seed),
            first: i,
            pos: 0,
        }
    }
}

/// Play `i` now (a choice, or an allowed autostart), unless it's in the player already; shuffling
/// starts a round from it.
fn choose(s: Playlist, i: u32) -> (Playlist, Command) {
    if i >= s.count || (s.live && s.playing == Some(i)) {
        return (s, Command::Nothing);
    }
    let round = s.settings.shuffle.then(|| Round {
        seed: mix(s.round.map_or(u64::from(i), |r| r.seed)),
        first: i,
        pos: 0,
    });
    let next = Playlist {
        playing: Some(i),
        live: true,
        round,
        ..s
    };
    (next, Command::Play(i))
}

/// Move on by list or round order; past the end, Repeat All (or One, which never holds a move on
/// the same scene) wraps and Repeat Off stops.
fn forward(s: Playlist) -> (Playlist, Command) {
    let wraps = s.settings.repeat != Repeat::Off;
    match (s.round, s.playing) {
        (None, None) => go(s, 0, None),
        (None, Some(p)) if p + 1 < s.count => go(s, p + 1, None),
        (None, Some(_)) if wraps => go(s, 0, None),
        (Some(r), None) => go(s, order(&r, s.count, r.pos), Some(r)),
        (Some(r), Some(_)) if r.pos + 1 < s.count => {
            let r = Round {
                pos: r.pos + 1,
                ..r
            };
            go(s, order(&r, s.count, r.pos), Some(r))
        }
        (Some(r), Some(_)) if wraps => {
            let r = fresh_round(mix(r.seed), s.count, s.playing);
            go(s, r.first, Some(r))
        }
        _ => (s, Command::Stop),
    }
}

fn backward(s: Playlist) -> (Playlist, Command) {
    match (s.round, s.playing) {
        (None, Some(p)) if p > 0 => go(s, p - 1, None),
        (Some(r), Some(_)) if r.pos > 0 => {
            let r = Round {
                pos: r.pos - 1,
                ..r
            };
            go(s, order(&r, s.count, r.pos), Some(r))
        }
        _ => (s, Command::Nothing),
    }
}

/// Play `target` (with its round); the playing scene itself is a replay.
fn go(s: Playlist, target: u32, round: Option<Round>) -> (Playlist, Command) {
    let cmd = if s.playing == Some(target) {
        Command::Replay
    } else {
        Command::Play(target)
    };
    let next = Playlist {
        playing: Some(target),
        live: true,
        round,
        ..s
    };
    (next, cmd)
}

/// A new round from the seed's own first pick, avoiding `avoid` (the scene that just played) when
/// there's another to choose.
fn fresh_round(seed: u64, count: u32, avoid: Option<u32>) -> Round {
    let p0 = permute(seed, count, 0);
    let first = if count > 1 && avoid == Some(p0) {
        permute(seed, count, 1)
    } else {
        p0
    };
    Round {
        seed,
        first,
        pos: 0,
    }
}

/// The scene at position `k` of the round (`k < count`): O(1) time and memory.
pub fn order(round: &Round, count: u32, k: u32) -> u32 {
    if k == 0 {
        return round.first;
    }
    let f = unpermute(round.seed, count, round.first);
    let j = k - 1;
    permute(round.seed, count, if j >= f { j + 1 } else { j })
}

/// The position of scene `i` in the round: the inverse of [`order`].
pub fn position_of(round: &Round, count: u32, i: u32) -> u32 {
    if i == round.first {
        return 0;
    }
    let f = unpermute(round.seed, count, round.first);
    let j = unpermute(round.seed, count, i);
    if j < f {
        j + 1
    } else {
        j
    }
}

/// A splitmix64 step: the deterministic seed for the next round, and the Feistel round function.
pub fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

const ROUNDS: u64 = 4;

/// Half the bits of the smallest even-width power of two covering `0..count`.
fn half_bits(count: u32) -> u32 {
    let bits = (64 - u64::from(count - 1).leading_zeros()).max(1);
    bits.div_ceil(2)
}

fn feistel(seed: u64, half: u32, x: u64, forward: bool) -> u64 {
    let mask = (1u64 << half) - 1;
    let (mut l, mut r) = (x >> half, x & mask);
    let key = |i: u64| mix(seed ^ mix(i));
    if forward {
        for i in 0..ROUNDS {
            (l, r) = (r, l ^ (mix(r ^ key(i)) & mask));
        }
    } else {
        for i in (0..ROUNDS).rev() {
            (l, r) = (r ^ (mix(l ^ key(i)) & mask), l);
        }
    }
    (l << half) | r
}

/// A seeded bijection on `0..count`: a Feistel network over the covering power of two, walking
/// the cycle until it lands below `count`.
fn permute(seed: u64, count: u32, i: u32) -> u32 {
    walk(seed, count, i, true)
}

fn unpermute(seed: u64, count: u32, i: u32) -> u32 {
    walk(seed, count, i, false)
}

fn walk(seed: u64, count: u32, i: u32, forward: bool) -> u32 {
    if count <= 1 {
        return i;
    }
    let half = half_bits(count);
    let mut x = u64::from(i);
    loop {
        x = feistel(seed, half, x, forward);
        if x < u64::from(count) {
            return x as u32;
        }
    }
}
