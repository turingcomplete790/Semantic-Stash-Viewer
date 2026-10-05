//! A wrapper around a scrollable that reports its position sparingly (007 T041).
//!
//! iced rebuilds and lays out the whole view after every message, and a scrollable's `on_scroll`
//! sends one on every frame of scrolling. For the Scenes grid that's the cost of scrolling, though
//! nothing visible changed. This wrapper watches the scrollable's offset itself and reports it
//! only when it crosses a row (so the rows built around the view can move) and once scrolling
//! has settled (so the saved position is exact). Frames in between only redraw.
//!
//! It also runs the UI bench's scroll (`AutoScroll`): scrolling the scrollable a fixed distance
//! per second, frame by frame, and timing the frames, with no messages per frame, exactly as a
//! person scrolling would.

use std::time::{Duration, Instant};

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::operation::scrollable::{AbsoluteOffset, Scrollable};
use iced::advanced::widget::{tree, Id, Operation, Tree};
use iced::advanced::{mouse, overlay, renderer, Clipboard, Shell, Widget};
use iced::{window, Element, Event, Length, Rectangle, Size, Vector};

/// How long the offset must stay put before the exact position is reported.
const SETTLE: Duration = Duration::from_millis(150);
/// Idle frames timed before an automatic scroll, for the display's frame time.
const BASELINE_FRAMES: usize = 60;

/// A bench scroll to run once (a new `id` runs again).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoScroll {
    pub id: u64,
    pub px_per_second: f32,
    pub duration: Duration,
}

/// Frame intervals (ms) from an automatic scroll: idle first, then scrolling.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoScrolled {
    pub id: u64,
    pub baseline: Vec<f64>,
    pub frames: Vec<f64>,
}

pub struct ScrollWatch<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    step: f32,
    /// Where the scrollable starts when this widget is first built (the saved position).
    restore: f32,
    on_scroll: Box<dyn Fn(f32) -> Message + 'a>,
    auto: Option<(AutoScroll, OnAutoScrolled<'a, Message>)>,
}

type OnAutoScrolled<'a, Message> = Box<dyn Fn(AutoScrolled) -> Message + 'a>;

/// Watch `content`'s first scrollable; report its offset when it moves into another `step`-sized
/// band, and when it settles. When first built, the scrollable is moved to `restore` (the
/// position the state remembers), so the two can't disagree however the view was rebuilt.
pub fn scroll_watch<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    step: f32,
    restore: f32,
    on_scroll: impl Fn(f32) -> Message + 'a,
) -> ScrollWatch<'a, Message, Theme, Renderer> {
    ScrollWatch {
        content: content.into(),
        step: step.max(1.0),
        restore: restore.max(0.0),
        on_scroll: Box::new(on_scroll),
        auto: None,
    }
}

impl<'a, Message, Theme, Renderer> ScrollWatch<'a, Message, Theme, Renderer> {
    /// Run a bench scroll, reporting the frame timings with `done`.
    pub fn auto_scroll(
        mut self,
        auto: Option<AutoScroll>,
        done: impl Fn(AutoScrolled) -> Message + 'a,
    ) -> Self {
        self.auto = auto.map(|a| (a, Box::new(done) as OnAutoScrolled<'a, Message>));
        self
    }
}

struct Run {
    id: u64,
    last: Option<Instant>,
    baseline: Vec<f64>,
    frames: Vec<f64>,
    started: Option<Instant>,
    y: f32,
}

#[derive(Default)]
struct State {
    /// The first event has been seen (the saved position restored).
    started: bool,
    band: Option<i64>,
    offset: f32,
    /// When the offset last changed, while it hasn't been reported settled.
    moving_since: Option<Instant>,
    run: Option<Run>,
    finished: Option<u64>,
}

/// Reads (and optionally sets) the first scrollable's vertical offset.
struct Offset {
    set: Option<f32>,
    found: Option<(f32, f32)>,
}

impl<T> Operation<T> for Offset {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        if self.found.is_none() {
            operate(self);
        }
    }

    fn scrollable(
        &mut self,
        _id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        if self.found.is_some() {
            return;
        }
        let max = (content_bounds.height - bounds.height).max(0.0);
        if let Some(y) = self.set {
            state.scroll_to(AbsoluteOffset {
                x: None,
                y: Some(y.clamp(0.0, max)),
            });
        }
        self.found = Some((translation.y, max));
    }
}

impl<Message, Theme, Renderer> ScrollWatch<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn offset(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        set: Option<f32>,
    ) -> Option<(f32, f32)> {
        let mut op = Offset { set, found: None };
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, &mut op);
        op.found
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ScrollWatch<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        let now = match event {
            Event::Window(window::Event::RedrawRequested(at)) => Some(*at),
            _ => None,
        };

        // First event since this widget was built: put the scrollable where the state says it
        // is. (Restoring through a separate task could run before the rebuilt view existed,
        // leaving the scrollable at the top while the rows built were the remembered ones: an
        // empty band where the grid should be.)
        let mut set = None;
        let first = {
            let state = tree.state.downcast_mut::<State>();
            let first = !state.started;
            state.started = true;
            first
        };
        if first && self.auto.is_none() {
            set = Some(self.restore);
        }
        if let (Some(now), Some((auto, done))) = (now, self.auto.as_ref()) {
            let state = tree.state.downcast_mut::<State>();
            if state.finished != Some(auto.id) {
                let run = state.run.get_or_insert_with(|| Run {
                    id: auto.id,
                    last: None,
                    baseline: Vec::new(),
                    frames: Vec::new(),
                    started: None,
                    y: 0.0,
                });
                if run.id != auto.id {
                    *run = Run {
                        id: auto.id,
                        last: None,
                        baseline: Vec::new(),
                        frames: Vec::new(),
                        started: None,
                        y: 0.0,
                    };
                }
                let interval = run
                    .last
                    .map(|t| now.duration_since(t).as_secs_f64() * 1000.0);
                run.last = Some(now);
                match (run.started, interval) {
                    (None, Some(i)) => {
                        run.baseline.push(i);
                        if run.baseline.len() >= BASELINE_FRAMES {
                            run.started = Some(now);
                        }
                    }
                    (Some(started), Some(i)) => {
                        run.frames.push(i);
                        if now.duration_since(started) >= auto.duration {
                            let run = state.run.take().expect("running");
                            state.finished = Some(auto.id);
                            shell.publish(done(AutoScrolled {
                                id: run.id,
                                baseline: run.baseline,
                                frames: run.frames,
                            }));
                        } else {
                            run.y += auto.px_per_second * (i as f32 / 1000.0);
                            set = Some(run.y);
                        }
                    }
                    _ => {}
                }
                shell.request_redraw();
            }
        }

        let Some((offset, max)) = self.offset(tree, layout, renderer, set) else {
            return;
        };
        if let (Some(y), Some(run)) = (set, tree.state.downcast_mut::<State>().run.as_mut()) {
            // Wrap at the end, as the web bench did.
            if y >= max {
                run.y = 0.0;
            }
        }
        let offset = set.map_or(offset, |y| y.clamp(0.0, max));

        let state = tree.state.downcast_mut::<State>();
        let band = (offset / self.step).floor() as i64;
        if first {
            state.offset = offset;
            state.band = Some(band);
            // The scrollable couldn't go all the way (the content is shorter now): report where
            // it is.
            if (offset - self.restore).abs() > 0.5 {
                shell.publish((self.on_scroll)(offset));
            }
            return;
        }
        if offset != state.offset {
            state.offset = offset;
            state.moving_since = Some(Instant::now());
            if state.band != Some(band) {
                state.band = Some(band);
                shell.publish((self.on_scroll)(offset));
            }
            shell.request_redraw_at(Instant::now() + SETTLE);
        } else if let Some(since) = state.moving_since {
            if since.elapsed() >= SETTLE {
                state.moving_since = None;
                shell.publish((self.on_scroll)(offset));
            } else {
                shell.request_redraw_at(since + SETTLE);
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a, Theme: 'a, Renderer> From<ScrollWatch<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer + 'a,
{
    fn from(w: ScrollWatch<'a, Message, Theme, Renderer>) -> Self {
        Element::new(w)
    }
}
