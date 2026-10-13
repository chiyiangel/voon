/// What the current hog-mode PID means for this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HogDisposition {
    /// The property is -1 (or another negative sentinel): nobody holds the device.
    Free,
    /// The recorded PID is this process.
    OwnedByUs,
    /// The recorded PID is not running.
    Stale { pid: i32 },
    /// A live process holds exclusive access.
    Busy { pid: i32 },
}

/// Classify a hog-mode PID.
///
/// `owner_alive` is ignored when `current_pid` is negative or equal to `our_pid`.
/// A recycled PID that now belongs to an unrelated live process is `Busy`; we do not steal it.
pub fn classify_hog(current_pid: i32, our_pid: i32, owner_alive: bool) -> HogDisposition {
    if current_pid < 0 {
        HogDisposition::Free
    } else if current_pid == our_pid {
        HogDisposition::OwnedByUs
    } else if owner_alive {
        HogDisposition::Busy { pid: current_pid }
    } else {
        HogDisposition::Stale { pid: current_pid }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_pid_is_free() {
        assert_eq!(classify_hog(-1, 10, true), HogDisposition::Free);
    }

    #[test]
    fn dead_owner_is_stale_and_live_owner_is_busy() {
        assert_eq!(
            classify_hog(42, 10, false),
            HogDisposition::Stale { pid: 42 }
        );
        assert_eq!(classify_hog(42, 10, true), HogDisposition::Busy { pid: 42 });
    }

    #[test]
    fn our_pid_is_not_treated_as_stale() {
        assert_eq!(classify_hog(10, 10, false), HogDisposition::OwnedByUs);
    }
}
