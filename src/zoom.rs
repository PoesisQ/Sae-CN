//! Event-driven PDF zoom. Drag travel is measured from the initial press.
const MIN: f32 = 0.25;
const MAX: f32 = 8.0;
const LOG_PER_PIXEL: f32 = std::f32::consts::LN_2 / 144.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stop {
    Free = 0,
    Page = 1,
    Fill = 2,
    Width = 3,
}

#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub value: f32,
    pub stop: Stop,
}

pub fn width_stop(page: (f32, f32), column: f32, height: f32) -> f32 {
    let wide = column / page.0.max(1.0);
    (wide / wide.min(height / page.1.max(1.0)).max(0.0001)).max(1.0)
}

pub fn fill_stop(page: (f32, f32), column: f32, height: f32) -> f32 {
    let (wide, tall) = (column / page.0.max(1.0), height / page.1.max(1.0));
    (wide.max(tall) / wide.min(tall).max(0.0001)).max(1.0)
}

pub fn position(value: f32) -> Position {
    Position {
        value,
        stop: if value == 1.0 { Stop::Page } else { Stop::Free },
    }
}

// On a fixed page, either Fit Width equals Fit Page or it equals Fill.
// Use one physical detent for each distinct scale.
fn detents(fill: f32) -> impl DoubleEndedIterator<Item = Position> {
    [
        position(1.0),
        Position {
            value: fill,
            stop: Stop::Fill,
        },
    ]
    .into_iter()
    .filter(|p| p.stop == Stop::Page || (1.035..=MAX).contains(&p.value))
}

pub fn step(current: f32, fill: f32, factor: f32) -> Position {
    if !factor.is_finite() || factor <= 0.0 || factor == 1.0 {
        return position(current);
    }
    let next = (current * factor).clamp(MIN, MAX);
    // Take a normal notch at a stop, then move freely on the next notch.
    // A large wheel gesture retains its full distance.
    if (0.84..=1.20).contains(&factor) {
        let mut stops = detents(fill);
        let stop = if factor > 1.0 {
            stops.find(|p| next >= p.value && current < p.value)
        } else {
            stops.rfind(|p| next <= p.value && current > p.value)
        };
        if let Some(stop) = stop {
            return stop;
        }
    }
    position(next)
}

#[derive(Clone, Copy)]
pub struct Drag {
    origin: f32,
    fill: f32,
    radius: f32,
}

impl Drag {
    pub fn start(current: f32, fill: f32) -> Self {
        let log = current.clamp(MIN, MAX).ln();
        let radius = if (1.035..=MAX).contains(&fill) {
            0.06_f32.min(fill.ln() * 0.49)
        } else {
            0.06
        };
        // Invert the response at the press point so picking up the knob
        // close to a detent does not alter the current scale.
        let mut origin = log;
        for p in detents(fill) {
            let target = p.value.ln();
            let offset = log - target;
            if offset != 0.0 && offset.abs() < radius {
                origin = target + offset * 0.8 + offset.signum() * radius * 0.2;
                break;
            }
        }
        Self {
            origin,
            fill,
            radius,
        }
    }

    pub fn at(self, travel: f32) -> Position {
        let raw = self.origin + travel * LOG_PER_PIXEL;
        for p in detents(self.fill) {
            let target = p.value.ln();
            let offset = raw - target;
            if offset.abs() < self.radius {
                let excess = offset.abs() - self.radius * 0.2;
                return if excess <= 0.0 {
                    p
                } else {
                    position(
                        (target + excess.copysign(offset) / 0.8)
                            .exp()
                            .clamp(MIN, MAX),
                    )
                };
            }
        }
        position(raw.exp().clamp(MIN, MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_is_monotonic_reversible_and_crosses_both_stops() {
        let dial = Drag::start(0.91, 1.72);
        let mut previous = dial.at(0.0).value;
        let mut page = false;
        let mut fill = false;
        for i in 1..300 {
            let p = dial.at(i as f32);
            assert!(p.value + 1e-6 >= previous);
            previous = p.value;
            page |= p.stop == Stop::Page;
            fill |= p.stop == Stop::Fill;
        }
        assert!(page && fill);
        // Away from the stop's neighborhood, travel covers full octaves.
        assert!((dial.at(288.0).value - 3.64).abs() < 1e-5);
        assert!((dial.at(0.0).value - 0.91).abs() < 1e-5);
        assert_eq!(dial.at(800.0).value, MAX);
        assert_eq!(dial.at(-800.0).value, MIN);
    }

    #[test]
    fn picking_up_the_dial_close_to_a_stop_keeps_its_position() {
        for value in [0.95, 0.99, 1.0, 1.014, 1.70, 1.72, 1.74] {
            assert!((Drag::start(value, 1.72).at(0.0).value - value).abs() < 1e-6);
        }
    }

    #[test]
    fn wheel_passes_stops_in_both_directions_and_keeps_fast_input() {
        assert_eq!(step(0.96, 1.8, 1.12).stop, Stop::Page);
        assert!((step(1.0, 1.8, 1.12).value - 1.12).abs() < 1e-6);
        assert_eq!(step(1.7, 1.8, 1.12).stop, Stop::Fill);
        assert_eq!(step(1.9, 1.8, 1.0 / 1.12).stop, Stop::Fill);
        assert!(step(1.8, 1.8, 1.0 / 1.12).value < 1.8);
        assert_eq!(step(0.9, 1.8, 3.0).value, 0.9 * 3.0);
        assert_eq!(step(7.9, 1.8, 2.0).value, MAX);
        assert_eq!(step(0.26, 1.8, 0.5).value, MIN);
        assert!((width_stop((612.0, 792.0), 520.0, 720.0) - 1.0).abs() < 0.001);
        assert!(width_stop((612.0, 792.0), 1100.0, 720.0) > 1.9);
        assert!(fill_stop((612.0, 792.0), 520.0, 720.0) > 1.06);
    }
}
