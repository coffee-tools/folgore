//! Recovery Algorithms for the plugin
//! instead to just return an error.
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::cell::RefCell;
use std::ops::{AddAssign, MulAssign};
use std::time::Duration;

use folgore_common::cln::plugin;
use folgore_common::cln::plugin::errors::PluginError;
use folgore_common::prelude::log;
use folgore_common::stragegy::RecoveryStrategy;
use folgore_common::Result;

/// Timeout Retry is a simple strategy that retry the call
/// for more times with a increasing timeout.
///
/// This is useful in case of HTTPs API services that block
/// a client request due the too many request in a range of
/// period.
///
/// Esplora implement something similar, and we work around
/// with this strategy.
pub struct TimeoutRetry {
    pub(crate) timeout: RefCell<Duration>,
    pub(crate) retry_state: RefCell<u8>,
    pub(crate) times: u8,
}

// SAFETY: All the backend request and blocking
// so there is not way to fall in Sync problem (also because the plugin is single thread)
unsafe impl Sync for TimeoutRetry {}

impl TimeoutRetry {
    pub fn new(duration: Option<Duration>) -> Self {
        Self {
            timeout: RefCell::new(duration.unwrap_or(Duration::from_secs(60))),
            retry_state: RefCell::new(0),
            times: 4,
        }
    }
}

impl Default for TimeoutRetry {
    fn default() -> Self {
        Self::new(None)
    }
}

impl RecoveryStrategy for TimeoutRetry {
    fn apply<T: Sized, F>(&self, cb: F) -> Result<T>
    where
        F: Fn() -> Result<T>,
    {
        let mut result = cb();
        while result.is_err() {
            log::info!(
                "running into retry logic due a request failing. Time `{}` waiting `{}` secs",
                self.times,
                self.timeout.borrow().as_secs()
            );
            if self.retry_state.borrow().eq(&self.times) {
                log::info!(
                    "we try {} times the request but the error persist",
                    *self.retry_state.borrow()
                );
                log::debug!(
                    "Error during the recovery strategy: `{:?}`",
                    result.as_ref().err()
                );
                // SAFETY: it is safe unwrap the error because we already know
                // that will be always Some,
                #[allow(clippy::unwrap_used)]
                return Err(plugin::error!(
                    "Recovery strategy (TimeoutRety) fails: `{}`",
                    result.err().unwrap().clone()
                ));
            }
            // This help us to keep the self not mutable.
            std::thread::sleep(*self.timeout.borrow());
            log::info!("Waiting timeout end");
            // now we increase the timeout
            self.timeout.borrow_mut().mul_assign(2);
            self.retry_state.borrow_mut().add_assign(1);
            result = cb();
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use folgore_common::prelude::cln_plugin::error;
    use folgore_common::prelude::cln_plugin::errors::PluginError;

    use super::{RecoveryStrategy, TimeoutRetry};

    use crate::configure_tests;

    #[test]
    fn test_simple_retry() {
        configure_tests();
        let strategy = TimeoutRetry::new(Some(Duration::from_millis(10)));

        let err: Result<(), PluginError> = strategy.apply(|| Err(error!("")));
        assert!(err.is_err());
    }

    #[test]
    fn test_state_strategy_one() {
        configure_tests();
        let strategy = TimeoutRetry::new(Some(Duration::from_millis(10)));

        let _: Result<(), PluginError> = strategy.apply(|| Err(error!("")));
        assert_eq!(*strategy.retry_state.borrow(), 4);
        let mut time = 10;
        for _ in 0..4 {
            time *= 2;
        }
        assert_eq!(*strategy.timeout.borrow(), Duration::from_millis(time));
    }

    #[test]
    fn test_state_strategy_two() {
        configure_tests();
        let strategy = TimeoutRetry::new(Some(Duration::from_millis(10)));

        let _: Result<(), PluginError> = strategy.apply(|| Ok(()));
        assert_eq!(*strategy.retry_state.borrow(), 0);
        assert_eq!(*strategy.timeout.borrow(), Duration::from_millis(10));
    }
}
