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

/// Compare real page fit, not an assumed paper size. Four pages are useful only
/// when they gain substantial occupied area without making each page much smaller.
pub fn pdf_grid_rows(sizes: &[(f32, f32)], width: f32, height: f32) -> i32 {
    if sizes.is_empty() || width < 800.0 || height < 600.0 {
        return 1;
    }
    let column = (width - 32.0) / 2.0;
    let cell = (height - 16.0) / 2.0;
    let mut areas = [0.0f32; 2];
    for &(w, h) in sizes {
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return 1;
        }
        let pair = (column / w).min(height / h);
        let grid = (column / w).min(cell / h);
        if grid < pair * 0.85 || column.min(w * grid) < 220.0 || h * grid < 150.0 {
            return 1;
        }
        areas[0] += w * h * pair * pair;
        areas[1] += w * h * grid * grid;
    }
    if areas[1] >= areas[0] * 0.73 { 2 } else { 1 }
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
/// Links and text drags are handled before this navigation rule is applied.
pub fn click_direction(spread: bool, slot: i32, x: f32, width: f32) -> i32 {
    if spread {
        if slot % 2 == 0 { -1 } else { 1 }
    } else if x < width * 0.5 {
        -1
    } else {
        1
    }
}
