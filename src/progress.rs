//! The thin loading bar across the top of the window.
//!
//! Instagram is a single-page application: tapping a profile or opening the
//! inbox never triggers a real page load, so WebKit's `load-changed` signal
//! stays silent and `estimated-load-progress` never moves. A bar driven only
//! by those would light up once at startup and never again.
//!
//! So the bar is driven by two sources at once:
//!
//!   * real page loads, where WebKit reports genuine progress, and
//!   * in-app navigation, detected from the URI changing, and finished when
//!     the network goes quiet.
//!
//! Neither source knows how much work is left, so between them the bar creeps
//! toward — but never reaches — [`CREEP_CEILING`], the same illusion YouTube
//! and GitHub use. It only completes when the load actually ends.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use webkit2gtk::{LoadEvent, WebResourceExt, WebViewExt};

/// How often the bar redraws while creeping.
const TICK: Duration = Duration::from_millis(60);
/// The bar never creeps past this on its own; only a finished load fills it.
const CREEP_CEILING: f64 = 0.92;
/// Fraction of the remaining distance covered per tick. Fast at first, slow
/// near the ceiling.
const CREEP_RATE: f64 = 0.055;
/// Where the bar jumps to the moment a navigation starts, so there is always
/// something visible immediately.
const INITIAL: f64 = 0.08;
/// How long the completed bar stays at 100% before disappearing.
const HOLD_AFTER_FINISH: Duration = Duration::from_millis(220);
/// In-app navigation is considered over once no request has been outstanding
/// for this long.
const QUIET_PERIOD: Duration = Duration::from_millis(400);

/// What one frame of the animation decided.
enum Step {
    /// Keep animating.
    Continue,
    /// The navigation is over; complete the bar.
    Done,
}

struct State {
    bar: gtk::ProgressBar,
    /// Highest fraction shown so far in this navigation. The bar must never
    /// jump backwards, which looks like an error even when it is not.
    fraction: f64,
    /// Real progress reported by WebKit, or `None` during in-app navigation.
    reported: Option<f64>,
    running: bool,
    /// Requests started but not yet finished or failed.
    in_flight: u32,
    /// Ticks with nothing in flight, used to detect the end of an in-app
    /// navigation.
    quiet_ticks: u32,
    /// Whether a tick source is currently installed.
    ticking: bool,
    /// Bumped on every begin and every finish. A pending "hide the bar" call
    /// checks it and does nothing if a new navigation started in the meantime.
    ///
    /// This is what replaces cancelling the timeout. Removing a GLib source by
    /// id is only safe while that source is alive and not currently
    /// dispatching; a one-shot timeout removes itself when it fires, so the id
    /// held here goes stale and GLib may well have handed it to somebody else
    /// by the time we would use it. Cancelling then destroys an unrelated
    /// source — including, on a busy page, one of WebKit's.
    generation: u64,
}

impl State {
    fn quiet_ticks_needed() -> u32 {
        (QUIET_PERIOD.as_millis() / TICK.as_millis()).max(1) as u32
    }

    fn begin(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if !self.running {
            self.running = true;
            self.fraction = INITIAL;
            self.reported = None;
            self.in_flight = 0;
            self.quiet_ticks = 0;
            self.bar.set_fraction(INITIAL);
            self.bar.show();
        }
    }

    /// Advances one frame.
    fn tick(&mut self) -> Step {
        if !self.running {
            return Step::Done;
        }

        // Creep toward the ceiling, and let real progress overrule it whenever
        // it is further along.
        let crept = self.fraction + (CREEP_CEILING - self.fraction) * CREEP_RATE;
        let target = match self.reported {
            Some(reported) => crept.max(reported.min(CREEP_CEILING)),
            None => crept,
        };
        if target > self.fraction {
            self.fraction = target;
            self.bar.set_fraction(self.fraction);
        }

        // Only in-app navigation ends this way; a real load ends on
        // `load-changed`.
        if self.reported.is_none() {
            if self.in_flight == 0 {
                self.quiet_ticks += 1;
                if self.quiet_ticks >= Self::quiet_ticks_needed() {
                    return Step::Done;
                }
            } else {
                self.quiet_ticks = 0;
            }
        }

        Step::Continue
    }
}

/// Fills the bar, then hides it a moment later — unless a new navigation has
/// started by then.
fn finish(state: &Rc<RefCell<State>>) {
    let generation = {
        let mut state = state.borrow_mut();
        if !state.running {
            return;
        }
        state.running = false;
        state.fraction = 1.0;
        state.bar.set_fraction(1.0);
        state.generation = state.generation.wrapping_add(1);
        state.generation
    };

    let state = state.clone();
    glib::timeout_add_local_once(HOLD_AFTER_FINISH, move || {
        let state = state.borrow();
        if state.generation == generation {
            state.bar.hide();
            state.bar.set_fraction(0.0);
        }
    });
}

/// Connects `bar` to `view`. The bar is expected to be an overlay child that
/// starts hidden.
pub fn install(view: &webkit2gtk::WebView, bar: &gtk::ProgressBar) {
    let state = Rc::new(RefCell::new(State {
        bar: bar.clone(),
        fraction: 0.0,
        reported: None,
        running: false,
        in_flight: 0,
        quiet_ticks: 0,
        ticking: false,
        generation: 0,
    }));

    // A real page load: WebKit reports genuine progress.
    {
        let state = state.clone();
        view.connect_load_changed(move |_, event| match event {
            LoadEvent::Started => {
                {
                    let mut borrowed = state.borrow_mut();
                    borrowed.begin();
                    borrowed.reported = Some(0.0);
                }
                start_ticking(&state);
            }
            LoadEvent::Finished => finish(&state),
            _ => {}
        });
    }

    {
        let state = state.clone();
        view.connect_estimated_load_progress_notify(move |view| {
            let progress = view.estimated_load_progress();
            let mut state = state.borrow_mut();
            if state.running {
                state.reported = Some(progress);
            }
        });
    }

    // In-app navigation: the URI changes without any page load.
    {
        let state = state.clone();
        view.connect_uri_notify(move |_| {
            {
                let mut borrowed = state.borrow_mut();
                if borrowed.running {
                    return;
                }
                borrowed.begin();
            }
            start_ticking(&state);
        });
    }

    // Network activity, used to decide when in-app navigation has settled.
    {
        let state = state.clone();
        view.connect_resource_load_started(move |_, resource, _| {
            state.borrow_mut().in_flight += 1;

            let done = {
                let state = state.clone();
                move || {
                    let mut state = state.borrow_mut();
                    state.in_flight = state.in_flight.saturating_sub(1);
                }
            };
            let on_failure = done.clone();
            resource.connect_finished(move |_| done());
            resource.connect_failed(move |_, _| on_failure());
        });
    }
}

fn start_ticking(state: &Rc<RefCell<State>>) {
    {
        let mut borrowed = state.borrow_mut();
        if borrowed.ticking {
            return;
        }
        borrowed.ticking = true;
    }

    let ticking = state.clone();
    // The source is never cancelled from outside; it retires itself by
    // returning `Break`, which is the only way that is safe from within a
    // dispatch.
    glib::timeout_add_local(TICK, move || {
        let step = ticking.borrow_mut().tick();
        match step {
            Step::Continue => glib::ControlFlow::Continue,
            Step::Done => {
                ticking.borrow_mut().ticking = false;
                finish(&ticking);
                glib::ControlFlow::Break
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_period_spans_several_ticks() {
        // A single quiet tick must not end a navigation, or the bar would
        // vanish in the gap between two requests.
        assert!(State::quiet_ticks_needed() >= 5);
    }

    #[test]
    fn creep_approaches_the_ceiling_without_reaching_it() {
        let mut fraction = INITIAL;
        for _ in 0..10_000 {
            fraction += (CREEP_CEILING - fraction) * CREEP_RATE;
        }
        assert!(fraction < CREEP_CEILING);
        assert!(fraction > CREEP_CEILING - 0.001);
    }
}
