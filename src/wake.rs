//! Waking the scene when something happened off the UI thread.
//!
//! A second launch, a click on a notification, a finished update check and a
//! termination signal all happen on threads of their own. The scene used to
//! learn about them by polling: four times a second with the window open, and
//! twelve and a half times a second with it closed — so that a launch from the
//! menu would show the hidden window quickly. A process sitting in the
//! background for a whole day woke a million times to find nothing.
//!
//! Now whoever has news calls [`Wake::wake`] after handing it over, and the
//! scene drains everything in one go. A burst of news is one wake-up: until
//! the scene has drained, further calls do nothing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

type Target = Box<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct Inner {
    /// A wake-up is on its way and the scene has not drained yet.
    queued: AtomicBool,
    /// What actually reaches the scene. Absent until the scene exists.
    target: Mutex<Option<Target>>,
}

/// A handle any thread may use to say "there is something for the scene".
#[derive(Clone, Default)]
pub struct Wake(Arc<Inner>);

impl Wake {
    /// Asks the scene to drain. Call it *after* handing the news over, so the
    /// drain it causes is certain to find it.
    pub fn wake(&self) {
        if self.0.queued.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Ok(target) = self.0.target.lock() {
            if let Some(target) = target.as_ref() {
                target();
            }
        }
    }

    /// Connects the handle to the scene. A wake-up asked for before this is
    /// delivered now rather than lost: the update check starts before the
    /// scene exists.
    pub fn attach(&self, target: impl Fn() + Send + Sync + 'static) {
        let Ok(mut held) = self.0.target.lock() else {
            return;
        };
        *held = Some(Box::new(target));
        if self.0.queued.load(Ordering::SeqCst) {
            if let Some(target) = held.as_ref() {
                target();
            }
        }
    }

    /// Called by the scene as it starts draining. Anything handed over from
    /// here on wakes it again.
    pub fn drained(&self) {
        self.0.queued.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    fn counting(wake: &Wake) -> Arc<AtomicU32> {
        let count = Arc::new(AtomicU32::new(0));
        let seen = Arc::clone(&count);
        wake.attach(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        count
    }

    #[test]
    fn a_burst_is_one_wake_up_until_the_scene_drains() {
        let wake = Wake::default();
        let count = counting(&wake);
        wake.wake();
        wake.wake();
        wake.wake();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        wake.drained();
        wake.wake();
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn news_from_before_the_scene_existed_is_not_lost() {
        let wake = Wake::default();
        wake.clone().wake();
        let count = counting(&wake);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn nothing_happens_without_news() {
        let wake = Wake::default();
        let count = counting(&wake);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
