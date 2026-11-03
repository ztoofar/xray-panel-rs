// Pure traffic-counter arithmetic shared by the polling job and the storage
// layer. Xray reports cumulative counters; the panel stores the last value it
// saw and bills the difference.

/// Interpret one cumulative counter sample against the stored baseline.
///
/// Returns `(new traffic, new baseline)`:
/// - `None` when the sample is missing or invalid (a negative counter). The
///   caller must keep the old baseline: treating a missing stat as zero would
///   later bill the whole counter again.
/// - a negative difference is a counter reset (xray restart, manual reset), so
///   the whole current value is fresh traffic.
pub fn counter_delta(current: Option<i64>, last: i64) -> Option<(i64, i64)> {
    let current = current?;
    if current < 0 {
        return None;
    }
    let last = last.max(0);
    if current >= last {
        Some((current - last, current))
    } else {
        Some((current, current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_negative_samples_preserve_the_baseline() {
        assert_eq!(counter_delta(None, 100), None);
        assert_eq!(counter_delta(Some(-1), 100), None);
        // A negative stored baseline is treated as zero instead of billing a
        // negative delta.
        assert_eq!(counter_delta(Some(5), -100), Some((5, 5)));
    }

    #[test]
    fn normal_growth_bills_the_difference() {
        assert_eq!(counter_delta(Some(150), 100), Some((50, 150)));
        assert_eq!(counter_delta(Some(100), 100), Some((0, 100)));
        assert_eq!(counter_delta(Some(0), 0), Some((0, 0)));
    }

    #[test]
    fn declined_counter_is_a_reset_and_bills_the_current_value() {
        assert_eq!(counter_delta(Some(10), 100), Some((10, 10)));
        // Exactly zero after a reset is a valid sample: bill nothing, move the
        // baseline down so the next growth is measured from zero.
        assert_eq!(counter_delta(Some(0), 100), Some((0, 0)));
    }
}
