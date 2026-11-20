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

/// Fixed presets share the same row-major geometry as the automatic overview.
pub const PDF_GRIDS: [(i32, i32); 5] = [(3, 1), (4, 1), (2, 2), (3, 2), (4, 2)];

pub fn pdf_grid(aspect: f32, width: f32, height: f32, choice: u8) -> (i32, i32) {
    if width < 800.0 {
        return (1, 1);
    }
    if let Some(&grid) = choice
        .checked_sub(1)
        .and_then(|i| PDF_GRIDS.get(i as usize))
    {
        return grid;
    }
    if !aspect.is_finite() || aspect <= 0.0 {
        return (2, 1);
    }
    let mut best = (2, 1);
    let mut best_score = 0.0;
    for (columns, rows) in PDF_GRIDS {
        let page_width = (width / columns as f32).min(height / rows as f32 * aspect);
        let page_height = page_width / aspect;
        if page_width < 220.0 || page_height < 150.0 {
            continue;
        }
        let count = (columns * rows) as f32;
        // Maximize actual paper area. A small density preference breaks near ties.
        let score = count * page_width * page_height * (1.0 + 0.025 * count);
        if score > best_score {
            best_score = score;
            best = (columns, rows);
        }
    }
    best
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
pub fn click_direction(columns: i32, slot: i32, x: f32, width: f32) -> i32 {
    let mid = 2 * (slot % columns) + 1;
    if mid < columns || (mid == columns && x < width * 0.5) {
        -1
    } else {
        1
    }
}
