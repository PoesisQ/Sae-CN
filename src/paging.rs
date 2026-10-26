//! Wheel gestures turn complete pages/spreads; they never alter a text offset.
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct WheelPager {
    last: Option<Instant>,
    turned: Option<Instant>,
    sum: f32,
    direction: i32,
    gesture_used: bool,
}
impl WheelPager {
    pub fn feed(&mut self, delta: f32, now: Instant) -> Option<i32> {
        if !delta.is_finite() || delta.abs() < 0.01 {
            return None;
        }
        let direction = if delta < 0.0 { 1 } else { -1 };
        if self
            .last
            .is_none_or(|last| now.duration_since(last) > Duration::from_millis(160))
            || direction != self.direction
        {
            self.sum = 0.0;
            self.gesture_used = false;
        }
        self.last = Some(now);
        self.direction = direction;
        if delta.abs() >= 40.0 {
            if self
                .turned
                .is_some_and(|last| now.duration_since(last) < Duration::from_millis(70))
            {
                return None;
            }
        } else {
            self.sum += delta.abs();
            if self.gesture_used || self.sum < 32.0 {
                return None;
            }
            self.gesture_used = true;
        }
        self.turned = Some(now);
        self.sum = 0.0;
        Some(direction)
    }
}
pub fn spread_start(page: i32) -> i32 {
    page.max(0) / 2 * 2
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_notch_or_trackpad_gesture_turns_one_whole_view() {
        let start = Instant::now();
        let mut pager = WheelPager::default();
        assert_eq!(pager.feed(-180.0, start), Some(1));
        assert_eq!(pager.feed(-180.0, start + Duration::from_millis(15)), None);
        assert_eq!(
            pager.feed(-180.0, start + Duration::from_millis(100)),
            Some(1)
        );
        let mut pager = WheelPager::default();
        let mut turns = 0;
        for i in 0..30 {
            turns += pager
                .feed(-4.0, start + Duration::from_millis(i * 10))
                .is_some() as i32;
        }
        assert_eq!(turns, 1);
        assert_eq!(
            pager.feed(180.0, start + Duration::from_millis(500)),
            Some(-1)
        );
        assert_eq!(spread_start(17), 16);
    }
}
