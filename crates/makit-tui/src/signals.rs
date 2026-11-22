//! Termination signals.
//!
//! In raw mode on the alternate screen, dying to a signal's default action
//! leaves the user's shell unusable. [`Signals::watch`] records SIGTERM,
//! SIGHUP and SIGINT for the event loop, which puts the terminal back and
//! then [`die`]s the way the signal asked.
//!
//! The loop cannot always answer. Once the terminal itself is gone (its
//! window was closed), crossterm's event poll spins on the dead tty and
//! never returns. So the thread that receives the signal is also a
//! watchdog: if the process is still alive [`GRACE`] later, it restores
//! what it can and ends the process itself.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
#[cfg(unix)]
use std::time::Duration;

/// How long the event loop gets to leave in order once a signal arrives.
/// It polls every 50 ms, so this is only ever used up when it is stuck.
#[cfg(unix)]
const GRACE: Duration = Duration::from_millis(500);
/// How long the watchdog waits for the terminal to take the restore
/// sequence; a terminal that has stopped reading would block it forever.
#[cfg(unix)]
const RESTORE_TIMEOUT: Duration = Duration::from_millis(250);

/// Termination signals received so far.
#[derive(Debug, Clone, Default)]
pub struct Signals {
    /// The last signal number received (0: none).
    received: Arc<AtomicUsize>,
}

impl Signals {
    /// Watch nothing.
    pub fn none() -> Self {
        Self::default()
    }

    /// Start recording termination signals instead of dying to them.
    #[cfg(unix)]
    pub fn watch() -> std::io::Result<Self> {
        Self::watch_with(GRACE, |signal| {
            restore_within(RESTORE_TIMEOUT);
            die(signal)
        })
    }

    /// Record termination signals. If the process is still alive `grace`
    /// after the first one, nothing answered it: call `give_up` with it.
    #[cfg(unix)]
    fn watch_with(
        grace: Duration,
        give_up: impl FnOnce(i32) + Send + 'static,
    ) -> std::io::Result<Self> {
        use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
        let mut incoming = signal_hook::iterator::Signals::new([SIGTERM, SIGHUP, SIGINT])?;
        let signals = Self::default();
        let received = signals.received.clone();
        std::thread::Builder::new()
            .name("makit-signals".into())
            .spawn(move || {
                let Some(signal) = incoming.forever().next() else {
                    return;
                };
                received.store(signal as usize, Ordering::Relaxed);
                std::thread::sleep(grace);
                give_up(signal);
            })?;
        Ok(signals)
    }

    /// Start recording termination signals (nothing to do on this platform).
    #[cfg(not(unix))]
    pub fn watch() -> std::io::Result<Self> {
        Ok(Self::default())
    }

    /// As if `signal` had already arrived.
    #[cfg(test)]
    pub(crate) fn already(signal: i32) -> Self {
        let signals = Self::default();
        signals.received.store(signal as usize, Ordering::Relaxed);
        signals
    }

    /// The signal that asked the process to end, if one has arrived.
    pub fn pending(&self) -> Option<i32> {
        match self.received.load(Ordering::Relaxed) {
            0 => None,
            signal => Some(signal as i32),
        }
    }
}

/// Put the terminal back from the watchdog, without waiting on it for long.
#[cfg(unix)]
fn restore_within(limit: Duration) {
    let (done, finished) = std::sync::mpsc::channel();
    let restoring = std::thread::Builder::new()
        .name("makit-restore".into())
        .spawn(move || {
            let _ = crate::terminal::restore();
            let _ = done.send(());
        });
    if restoring.is_ok() {
        let _ = finished.recv_timeout(limit);
    }
}

/// End the process the way `signal` would have. Call once the terminal is
/// back to normal.
pub fn die(signal: i32) -> ! {
    #[cfg(unix)]
    {
        let _ = signal_hook::low_level::emulate_default_handler(signal);
    }
    // Only reached if the default action did not end the process.
    std::process::exit(128 + signal)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use signal_hook::consts::SIGTERM;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    // One test on purpose: a raised signal reaches every handler in the
    // process, so a second test watching in parallel would see it too.
    #[test]
    fn a_sigterm_is_recorded_and_a_process_that_stays_alive_is_given_up_on() {
        let grace = Duration::from_millis(100);
        let (tx, rx) = mpsc::channel();
        let signals = Signals::watch_with(grace, move |signal| {
            let _ = tx.send(signal);
        })
        .expect("register handlers");
        assert_eq!(signals.pending(), None);

        let raised = Instant::now();
        signal_hook::low_level::raise(SIGTERM).unwrap();

        // Nothing ends this process, as when the event loop is stuck: the
        // watchdog steps in, but not before the grace period is over.
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(SIGTERM));
        assert!(raised.elapsed() >= grace);

        // The event loop was told, so it could have left in order first.
        assert_eq!(signals.pending(), Some(SIGTERM));
        // Clones share what was received; unwatched instances saw nothing.
        assert_eq!(signals.clone().pending(), Some(SIGTERM));
        assert_eq!(Signals::none().pending(), None);
    }
}
