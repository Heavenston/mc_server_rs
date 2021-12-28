use std::sync::{Condvar, Mutex};

pub struct AbortContract {
    is_aborted: Mutex<bool>,
    condvar: Condvar,
}

impl AbortContract {
    pub fn new() -> Self {
        Self {
            is_aborted: Mutex::new(false),
            condvar: Condvar::new(),
        }
    }

    /// returns true if the contract was aborted
    /// This will lock the inner Mutex
    pub fn is_aborted(&self) -> bool {
        *self.is_aborted.lock().unwrap()
    }
    /// Try to read the inner Mutex
    pub fn try_is_aborted(&self) -> Option<bool> {
        self.is_aborted.try_lock().map(|a| *a).ok()
    }

    /// Lock the Mutex and abort the contract
    /// Returns the previous state
    pub fn abort(&self) -> bool {
        let mut lock = self.is_aborted.lock().unwrap();
        if *lock {
            return true;
        }
        *lock = true;
        self.condvar.notify_all();
        false
    }
    /// Try to lock the Mutex and abort the contract
    /// Returns the previous state or None if the Locking wasn't successful
    pub fn try_abort(&self) -> Option<bool> {
        if let Ok(mut lock) = self.is_aborted.try_lock() {
            if *lock {
                return Some(true);
            }
            *lock = true;
            self.condvar.notify_all();
            Some(false)
        } else {
            None
        }
    }

    pub fn wait_for_abort(&self) {
        let _guard = self
            .condvar
            .wait_while(self.is_aborted.lock().unwrap(), |v| !*v)
            .unwrap();
    }
}
