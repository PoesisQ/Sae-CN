//! Event-driven zoom resistance. No animation clock, allocations or spring loop.
const MIN: f32 = -1.3862944; // ln(0.25)
const MAX: f32 = 2.0794415; // ln(8)
const RELEASE: f32 = 0.085;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stop {
    Free = 0,
    Page = 1,
    Fill = 2,
}
#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub value: f32,
    pub stop: Stop,
}
#[derive(Default)]
pub struct ZoomControl {
    raw: f32,
    last: f32,
    width: f32,
    latch: Option<(f32, f32, Stop)>,
}
pub fn width_stop(page: (f32, f32), column: f32, height: f32) -> f32 {
    let wide = column / page.0.max(1.0);
    (wide / wide.min(height / page.1.max(1.0)).max(0.0001)).max(1.0)
}
pub fn fill_stop(page: (f32, f32), column: f32, height: f32) -> f32 {
    let (wide, tall) = (column / page.0.max(1.0), height / page.1.max(1.0));
    (wide.max(tall) / wide.min(tall).max(0.0001)).max(1.0)
}
pub fn position(value: f32, width: f32) -> Position {
    Position {
        value,
        stop: if (value - 1.0).abs() < 0.001 {
            Stop::Page
        } else if width > 1.035 && (value / width - 1.0).abs() < 0.001 {
            Stop::Fill
        } else {
            Stop::Free
        },
    }
}
impl ZoomControl {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn adjust(&mut self, current: f32, width: f32, factor: f32) -> Position {
        if !factor.is_finite() || factor <= 0.0 || !current.is_finite() {
            return position(current, width);
        }
        let current = current.clamp(0.25, 8.0);
        if (self.last - current).abs() > 0.0001 || (self.width - width).abs() > 0.001 {
            self.raw = current.ln();
            self.latch = None;
        }
        self.width = width;
        let delta = factor.ln().clamp(-0.8, 0.8);
        let mut released = None;
        if let Some((target, debt, stop)) = self.latch {
            let debt = debt + delta;
            if debt.abs() <= RELEASE {
                self.latch = Some((target, debt, stop));
                self.last = target.exp();
                return Position {
                    value: self.last,
                    stop,
                };
            }
            self.raw = (target + debt.signum() * (debt.abs() - RELEASE) * 0.7).clamp(MIN, MAX);
            self.latch = None;
            released = Some(target);
        } else {
            let old = self.raw;
            let mut gain = 1.0f32;
            for (target, stop) in [(0.0, Stop::Page), (width.ln(), Stop::Fill)] {
                if stop == Stop::Fill && !(1.035..=8.0).contains(&width) {
                    continue;
                }
                let distance = target - old;
                if distance * delta > 0.0 && distance.abs() < 0.13 {
                    gain = gain.min(0.42 + 0.58 * distance.abs() / 0.13);
                }
            }
            self.raw = (old + delta * gain).clamp(MIN, MAX);
            let mut candidates = [(0.0, Stop::Page), (width.ln(), Stop::Fill)];
            if delta < 0.0 {
                candidates.swap(0, 1);
            }
            for (target, stop) in candidates {
                if stop == Stop::Fill && !(1.035..=8.0).contains(&width) {
                    continue;
                }
                let crossed =
                    (old < target && self.raw >= target) || (old > target && self.raw <= target);
                let approaching = (target - old) * delta > 0.0;
                if crossed
                    || approaching
                        && (self.raw - target).abs() < 0.018
                        && (old - target).abs() > 0.0001
                {
                    self.raw = target;
                    self.latch = Some((target, 0.0, stop));
                    self.last = target.exp();
                    return Position {
                        value: self.last,
                        stop,
                    };
                }
            }
        }
        // Releasing a stop must not immediately recapture the same magnet.
        let mut result = position(self.raw.exp().clamp(0.25, 8.0), width);
        if released.is_some() {
            result.stop = Stop::Free;
        }
        self.last = result.value;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn leaving_reset_has_no_phantom_resistance_and_circle_drag_catches_fill() {
        let mut control = ZoomControl::default();
        assert!((control.adjust(1.0, 1.8, 1.12).value - 1.12).abs() < 0.00001);
        control.reset();
        let result = control.adjust(1.0, 1.1290323, 2.0f32.powf(1.0 / 3.0));
        assert_eq!(result.stop, Stop::Fill);
        assert!((result.value - 1.1290323).abs() < 0.00001);
    }
    #[test]
    fn page_and_width_stops_catch_crossings_then_release_in_both_directions() {
        let mut dial = ZoomControl::default();
        let mut value = 0.91;
        let mut stops = Vec::new();
        for _ in 0..30 {
            let p = dial.adjust(value, 1.72, 1.12);
            value = p.value;
            stops.push(p.stop);
        }
        assert!(stops.contains(&Stop::Page) && stops.contains(&Stop::Fill));
        assert!(value > 2.0);
        stops.clear();
        for _ in 0..40 {
            let p = dial.adjust(value, 1.72, 1.0 / 1.12);
            value = p.value;
            stops.push(p.stop);
        }
        assert!(stops.contains(&Stop::Page) && stops.contains(&Stop::Fill));
        assert!(value < 0.8);
        assert!(
            stops.iter().filter(|s| **s != Stop::Free).count() < 12,
            "unreasonably sticky"
        );
    }
    #[test]
    fn fine_input_meets_resistance_without_getting_stuck_or_reversing_direction() {
        let mut dial = ZoomControl::default();
        let mut value = 0.94;
        let mut page = false;
        for _ in 0..100 {
            let p = dial.adjust(value, 1.8, 1.005);
            assert!(p.value >= value - 0.0001);
            value = p.value;
            page |= p.stop == Stop::Page;
        }
        assert!(page && value > 1.2);
        assert!((width_stop((612.0, 792.0), 520.0, 720.0) - 1.0).abs() < 0.001);
        assert!(width_stop((612.0, 792.0), 1100.0, 720.0) > 1.9);
        assert!(fill_stop((612.0, 792.0), 520.0, 720.0) > 1.06);
        for _ in 0..80 {
            value = dial.adjust(value, 1.0, 1.5).value;
        }
        assert_eq!(value, 8.0);
        for _ in 0..80 {
            value = dial.adjust(value, 1.0, 0.7).value;
        }
        assert_eq!(value, 0.25);
    }
}
