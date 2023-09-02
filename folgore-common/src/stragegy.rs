//! Recovery Strategy patter implementation for recovery methods
//! for a request and do not abort the plugin.
//!
//! If you do not know what Strategy patter is here is a small
//! description.
//!
//! > The basic idea behind the Strategy pattern is that, given an
//! > algorithm solving a particular problem,  we define only
//! > the skeleton of the algorithm at an abstract level, and we
//! > separate the specific algorithm’s implementation into
//! > different parts.
//! >
//! > In this way, a client using the algorithm may choose
//! > a specific implementation, while the general algorithm
//! > workflow remains the same. In other words, the abstract
//! > specification of the class does not depend on the specific
//! > implementation of the derived class, but specific implementation
//! > must adhere to the abstract specification.
//!
//! So in this specific case the nurse command may need
//! different kind of recovery algorithm, so we can have different
//! strategy of recovery from an error. This will be used when
//! a request fails and core lightning do not admit failure.
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use crate::Result;

pub trait RecoveryStrategy: Send + Sync {
    /// Apply the algorithm implemented by
    /// the kind of recovery strategy,
    fn apply<T, F>(&self, cb: F) -> Result<T>
    where
        F: Fn() -> Result<T>;
}
