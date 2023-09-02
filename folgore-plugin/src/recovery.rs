//! Recovery Algorithms for the plugin
//! instead to just return an error.
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::cell::RefCell;
use std::ops::AddAssign;
use std::time::Duration;

use folgore_common::cln_plugin::error;
use folgore_common::cln_plugin::errors::PluginError;
use folgore_common::stragegy::RecoveryStrategy;
use folgore_common::Result;

pub struct TimeoutRetry {
    pub timeout: RefCell<Duration>,
    pub times: RefCell<u8>,
}

// SAFETY: All the backend request and blocking
// so there is not way to fall in Sync problem (also because the plugin is single thread)
unsafe impl Sync for TimeoutRetry {}

impl TimeoutRetry {
    pub fn new(duration: Option<Duration>) -> Self {
        Self {
            timeout: RefCell::new(duration.unwrap_or(Duration::from_secs(60))),
            times: RefCell::new(4),
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
            if self.times.borrow().eq(&4) {
                // SAFETY: it is safe unwrap the error because we already know
                // that will be always Some,
                #[allow(clippy::unwrap_used)]
                return Err(error!(
                    "Recovery strategy (TimeoutRety) fails: `{}`",
                    result.err().unwrap().clone()
                ));
            }
            // This help us to keep the self not mutable.
            let timeout = *self.timeout.borrow();
            std::thread::sleep(timeout);
            // now we increase the timeout
            *self.timeout.borrow_mut() = timeout * 2;
            self.times.borrow_mut().add_assign(1);
            result = cb();
        }
        result
    }
}
