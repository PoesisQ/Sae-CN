//! PDF zoom responds to pointer events only. Turns and straight pulls share one scale and the same detents.
const MIN: f32 = 0.25;
const MAX: f32 = 8.0;
const LOG_PER_PIXEL: f32 = std::f32::consts::LN_2 / 144.0;

#[derive(Clone, Copy, PartialEq)]
enum Motion {
    Pending,
    Horizontal,
    Vertical,
    Circular,
}

/// Pointer positions use logical pixels around the axle captured on button-down.
/// An initial tangent fits both a pull and a turn. Keep that travel until a bend
/// or a straight chord distinguishes them, then keep the chosen grip to release.
pub struct Grip {
    start: (f32, f32),
    previous: (f32, f32),
    first_direction: Option<(f32, f32)>,
    radius: f32,
    sweep: f32,
    motion: Motion,
    travel: f32,
}

impl Grip {
    pub fn new(x: f32, y: f32, radius: f32) -> Self {
        Self {
            start: (x, y),
            previous: (x, y),
            first_direction: None,
            radius,
            sweep: 0.0,
            motion: Motion::Pending,
            travel: 0.0,
        }
    }

    fn angle_from_previous(&self, x: f32, y: f32) -> f32 {
        let (px, py) = self.previous;
        let hub = (self.radius * 0.26).powi(2);
        let cross = px * y - py * x;
        let dot = px * x + py * y;
        if px * px + py * py <= hub
            || x * x + y * y <= hub
            || (dot < 0.0 && cross.abs() < -0.5 * dot)
        {
            return 0.0;
        }
        cross.atan2(dot)
    }

    pub fn sample(&mut self, x: f32, y: f32, dial: Drag) -> Option<f32> {
        const ARC_GAIN: f32 = 144.0 / std::f32::consts::PI;
        let delta = match self.motion {
            Motion::Horizontal => x - self.previous.0,
            Motion::Vertical => self.previous.1 - y,
            Motion::Circular => self.angle_from_previous(x, y) * ARC_GAIN,
            Motion::Pending => {
                let (sx, sy) = self.start;
                let (dx, dy) = (x - sx, y - sy);
                let distance = dx.hypot(dy);
                self.sweep += self.angle_from_previous(x, y);
                self.previous = (x, y);
                if distance < 4.0 {
                    return None;
                }
                let initial_radius = sx.hypot(sy);
                let at_rim = self.radius > 0.0
                    && (0.36 * self.radius..=1.60 * self.radius).contains(&initial_radius);
                let first = *self
                    .first_direction
                    .get_or_insert((dx / distance, dy / distance));
                let bending = first.0 * dy - first.1 * dx;
                let r = x.hypot(y);
                // The second condition also handles sparse input where a quarter
                // turn arrives as one event. A straight tangent changes radius.
                let curved = bending * self.sweep.signum() >= (distance * 0.065).max(1.2);
                let diagonal = dx.abs().min(dy.abs()) > distance * 0.22;
                let follows_rim = diagonal
                    && (r - initial_radius).abs() < initial_radius * 0.13
                    && self.sweep.abs() >= 0.71;
                if at_rim && self.sweep.abs() >= 0.35 && (curved || follows_rim) {
                    self.motion = Motion::Circular;
                    self.sweep * ARC_GAIN
                } else {
                    let radial = (sx * dx + sy * dy).abs() > initial_radius * distance * 0.86;
                    if at_rim
                        && !(radial && distance >= 12.0 && self.sweep.abs() < 0.16)
                        && distance < initial_radius * 0.84
                        && r > self.radius * 0.26
                    {
                        return None;
                    }
                    if dx.abs() >= dy.abs() {
                        self.motion = Motion::Horizontal;
                        dx
                    } else {
                        self.motion = Motion::Vertical;
                        -dy
                    }
                }
            }
        };
        self.previous = (x, y);
        self.travel = (self.travel + delta).clamp(
            (MIN.ln() - dial.origin) / LOG_PER_PIXEL,
            (dial.presets.limit().ln() - dial.origin) / LOG_PER_PIXEL,
        );
        Some(self.travel)
    }
    pub fn edge_direction(&self, x: f32, y: f32, width: f32, height: f32) -> i8 {
        match self.motion {
            Motion::Horizontal if x >= width - 2.0 => 1,
            Motion::Horizontal if x <= 2.0 => -1,
            Motion::Vertical if y <= 2.0 => 1,
            Motion::Vertical if y >= height - 2.0 => -1,
            _ => 0,
        }
    }
    pub fn advance(&mut self, distance: f32, dial: Drag) -> Option<Position> {
        let next = (self.travel + distance).clamp(
            (MIN.ln() - dial.origin) / LOG_PER_PIXEL,
            (dial.presets.limit().ln() - dial.origin) / LOG_PER_PIXEL,
        );
        if next == self.travel {
            return None;
        }
        self.travel = next;
        Some(dial.at(next))
    }
}

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

impl Stop {
    pub fn from_fit(width: bool, fill: bool) -> Self {
        if fill {
            Self::Fill
        } else if width {
            Self::Width
        } else {
            Self::Page
        }
    }
}

/// Device pixels per PDF point, before the renderer applies its scale limits.
pub struct CellFit {
    wide: f32,
    tall: f32,
}

impl CellFit {
    pub fn new(page: (f32, f32), cell: (f32, f32), dpi: f32) -> Self {
        Self {
            wide: cell.0 / page.0.max(1.0) * dpi,
            tall: cell.1 / page.1.max(1.0) * dpi,
        }
    }

    pub fn scale(&self, stop: Stop, zoom: f32) -> f32 {
        let fit = match stop {
            Stop::Width => self.wide,
            Stop::Fill => self.wide.max(self.tall),
            _ => self.wide.min(self.tall),
        };
        (fit * zoom).clamp(0.02, 16.0)
    }
}

pub fn position(value: f32) -> Position {
    Position {
        value,
        stop: if value == 1.0 { Stop::Page } else { Stop::Free },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Presets {
    pub width: f32,
    pub fill: f32,
}

impl Presets {
    pub const PAGE: Self = Self {
        width: 1.0,
        fill: 1.0,
    };

    pub fn for_pages(pages: impl Iterator<Item = (f32, f32)>, cell: (f32, f32), dpi: f32) -> Self {
        let (mut width_gain, mut fill_gain, mut count) = (0.0, 0.0, 0);
        for page in pages {
            let fit = CellFit::new(page, cell, dpi);
            let base = fit.scale(Stop::Page, 1.0);
            let wide = fit.scale(Stop::Width, 1.0);
            let cover = fit.scale(Stop::Fill, 1.0);
            let longest = page.0.max(page.1);
            // A different fit must move at least half a device pixel on one
            // visible page. Identical results stay exactly equal after averaging.
            if (wide - base) * longest >= 0.5 {
                width_gain += (wide - base) / base;
            }
            if (cover - wide) * longest >= 0.5 {
                fill_gain += (cover - wide) / base;
            }
            count += 1;
        }
        let n = count.max(1) as f32;
        let width = 1.0 + width_gain / n;
        Self {
            width,
            fill: width + fill_gain / n,
        }
    }

    pub fn at(self, stop: Stop) -> Position {
        Position {
            value: match stop {
                Stop::Width => self.width,
                Stop::Fill => self.fill,
                _ => 1.0,
            },
            stop,
        }
    }

    pub fn canonical(self, stop: Stop) -> Stop {
        match stop {
            Stop::Fill if self.fill == self.width => self.canonical(Stop::Width),
            Stop::Width if self.width == 1.0 => Stop::Page,
            _ => stop,
        }
    }

    pub fn stops(self) -> impl DoubleEndedIterator<Item = Position> {
        [Stop::Page, Stop::Width, Stop::Fill]
            .into_iter()
            .filter(move |&s| self.canonical(s) == s)
            .map(move |s| self.at(s))
    }

    pub fn clicked(self, kind: i32, active: Stop) -> Option<Position> {
        let mark = self.stops().find(|p| p.stop as i32 == kind)?;
        // Re-centering through a merged mark keeps a sticky fit for the next
        // resize. An explicit fit-page command always resets that intent.
        Some(if self.canonical(active) == mark.stop {
            self.at(active)
        } else {
            mark
        })
    }

    fn limit(self) -> f32 {
        // Very narrow pages can genuinely need more than 800% to fill the
        // cell. A grip must be able to enter and leave their named fit.
        MAX.max(self.fill)
    }
}

pub fn step(current: f32, presets: Presets, factor: f32) -> Position {
    if !factor.is_finite() || factor <= 0.0 || factor == 1.0 {
        return position(current);
    }
    let next = (current * factor).clamp(MIN, presets.limit());
    // Take a normal notch at a stop, then move freely on the next notch.
    // A large wheel gesture retains its full distance.
    if (0.84..=1.20).contains(&factor) {
        let mut stops = presets.stops();
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
    presets: Presets,
    radius: f32,
}

impl Drag {
    pub fn start(current: f32, presets: Presets) -> Self {
        let log = current.clamp(MIN, presets.limit()).ln();
        let mut radius = 0.06_f32;
        let mut previous = 1.0;
        for p in presets.stops().skip(1) {
            radius = radius.min((p.value / previous).ln() * 0.49);
            previous = p.value;
        }
        // Invert the response at the press point so picking up the knob
        // close to a detent does not alter the current scale.
        let mut origin = log;
        for p in presets.stops() {
            let target = p.value.ln();
            let offset = log - target;
            if offset != 0.0 && offset.abs() < radius {
                origin = target + offset * 0.8 + offset.signum() * radius * 0.2;
                break;
            }
        }
        Self {
            origin,
            presets,
            radius,
        }
    }

    pub fn at(self, travel: f32) -> Position {
        let raw = self.origin + travel * LOG_PER_PIXEL;
        for p in self.presets.stops() {
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
                            .clamp(MIN, self.presets.limit()),
                    )
                };
            }
        }
        position(raw.exp().clamp(MIN, self.presets.limit()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cover(fill: f32) -> Presets {
        Presets { width: 1., fill }
    }

    #[test]
    fn marks_follow_real_visible_pages_and_dpi() {
        let (portrait, landscape, square) = ((500., 1000.), (1000., 500.), (600., 600.));
        let for_view =
            |pages: &[(f32, f32)], cell, dpi| Presets::for_pages(pages.iter().copied(), cell, dpi);
        let cell = (600., 600.);
        let one = for_view(&[square], cell, 1.5);
        let wide = for_view(&[portrait; 2], cell, 1.5);
        let fill = for_view(&[landscape; 6], cell, 1.5);
        let mixed = for_view(&[portrait, landscape], cell, 1.5);
        assert_eq!(
            one.stops().map(|p| p.stop).collect::<Vec<_>>(),
            [Stop::Page]
        );
        assert_eq!(
            wide.stops().map(|p| p.stop).collect::<Vec<_>>(),
            [Stop::Page, Stop::Width]
        );
        assert_eq!(
            fill.stops().map(|p| p.stop).collect::<Vec<_>>(),
            [Stop::Page, Stop::Fill]
        );
        assert_eq!(
            mixed.stops().map(|p| p.value).collect::<Vec<_>>(),
            [1., 1.5, 2.]
        );
        assert_eq!(for_view(&[landscape, portrait], cell, 1.5), mixed);
        assert_eq!(
            for_view(&[portrait, landscape], (300., 600.), 1.),
            cover(2.5)
        );
        assert_eq!(for_view(&[(500., 500.25)], (500., 500.), 1.), Presets::PAGE);
        assert!(for_view(&[(500., 500.25)], (500., 500.), 3.).width > 1.);
        // Both proposed fits are beyond the renderer's native scale cap.
        assert_eq!(for_view(&[(1., 2.)], cell, 1.), Presets::PAGE);
        assert_eq!(for_view(&[(50_000., 80_000.)], cell, 1.), Presets::PAGE);
    }

    #[test]
    fn merged_clicks_preserve_an_explicit_fit_and_ignore_missing_marks() {
        let together = cover(1.8);
        assert_eq!(
            together
                .clicked(Stop::Page as i32, Stop::Width)
                .unwrap()
                .stop,
            Stop::Width
        );
        assert!(together.clicked(Stop::Width as i32, Stop::Free).is_none());
        let resized = Presets {
            width: 1.6,
            fill: 1.6,
        };
        assert_eq!(resized.at(Stop::Width).value, 1.6);
        assert_eq!(
            resized
                .clicked(Stop::Width as i32, Stop::Fill)
                .unwrap()
                .stop,
            Stop::Fill
        );
        assert!(resized.clicked(Stop::Fill as i32, Stop::Free).is_none());
        assert_eq!(
            resized
                .clicked(Stop::Page as i32, Stop::Width)
                .unwrap()
                .stop,
            Stop::Page
        );
    }

    #[test]
    fn mixed_spread_hits_each_detent_and_extreme_presets_can_be_left() {
        let p = Presets {
            width: 1.5,
            fill: 2.0,
        };
        let dial = Drag::start(1., p);
        for mark in p.stops() {
            assert_eq!(dial.at(mark.value.ln() / LOG_PER_PIXEL).stop, mark.stop);
            assert_eq!(step(mark.value * 0.98, p, 1.05).stop, mark.stop);
            assert_eq!(step(mark.value * 1.02, p, 0.95).stop, mark.stop);
        }
        let long = Presets::for_pages([(1000., 50.)].into_iter(), (600., 600.), 1.);
        assert!((long.fill - 20.).abs() < 0.00001);
        let dial = Drag::start(long.fill, long);
        assert_eq!(dial.at(0.).value, long.fill);
        assert_eq!(dial.at(40.).value, long.fill);
        assert!((long.fill / dial.at(-144.).value - 2.).abs() < 0.001);
    }

    #[test]
    fn circular_wheel_keeps_straight_drag_distance_and_full_turn_gain() {
        let dial = Drag::start(1.0, cover(1.8));
        let radius = 43.0;
        let mut face = Grip::new(0.0, 0.0, radius);
        let travel = face.sample(1.0, -144.0, dial).unwrap();
        assert!((dial.at(travel).value - 2.0).abs() < 0.001);
        let mut rim = Grip::new(0.0, -36.0, radius);
        for step in 1..=24 {
            let a = step as f32 * std::f32::consts::TAU / 24.0;
            let travel = rim
                .sample(36.0 * a.sin(), -36.0 * a.cos(), dial)
                .unwrap_or(0.0);
            if step == 24 {
                assert!((dial.at(travel).value - 4.0).abs() < 0.001);
            }
        }
    }
    #[test]
    fn face_extends_at_screen_edge_and_reverses_immediately_at_the_limit() {
        let dial = Drag::start(1.0, cover(1.7));
        for (dx, dy, x, y, sign) in [
            (9., 0., 999., 400., 1),
            (-9., 0., 1., 400., -1),
            (0., -9., 400., 1., 1),
            (0., 9., 400., 799., -1),
        ] {
            let mut face = Grip::new(0., 0., 40.);
            face.sample(dx, dy, dial).unwrap();
            assert_eq!(face.edge_direction(x, y, 1000., 800.), sign);
            assert_eq!(face.edge_direction(400., 400., 1000., 800.), 0);
            let end = face.advance(f32::from(sign) * 2000., dial).unwrap();
            assert_eq!(end.value, if sign > 0 { MAX } else { MIN });
            assert!(face.advance(f32::from(sign) * 8., dial).is_none());
            assert_ne!(
                face.advance(-f32::from(sign) * 8., dial).unwrap().value,
                end.value
            );
        }
        let mut rim = Grip::new(36., 0., 40.);
        rim.sample(0., 36., dial).unwrap();
        assert_eq!(rim.edge_direction(999., 400., 1000., 800.), 0);
    }
    #[test]
    fn circular_turn_wraps_reverses_and_releases_at_the_limits() {
        let dial = Drag::start(1.0, cover(1.7));
        let mut rim = Grip::new(0.0, -34.0, 40.0);
        let mut turn = |degrees: i32| {
            let angle = (degrees as f32).to_radians();
            let travel = rim
                .sample(34.0 * angle.sin(), -34.0 * angle.cos(), dial)
                .unwrap_or(0.0);
            dial.at(travel).value
        };
        let mut prev = 1.0;
        for degrees in (12..=720).step_by(12) {
            let value = turn(degrees);
            assert!(value >= prev - 1e-5);
            if degrees == 360 {
                assert!((value - 4.0).abs() < 1e-4);
            }
            prev = value;
        }
        assert_eq!(prev, MAX);
        assert!(turn(708) < MAX);
        for degrees in (0..=696).rev().step_by(12) {
            turn(degrees);
        }
        assert!((turn(0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn face_stays_linear_and_crossing_the_hub_has_no_angle() {
        let dial = Drag::start(1.0, cover(1.7));
        let mut grip = Grip::new(0.0, 0.0, 40.0);
        assert_eq!(grip.sample(1.0, -1.0, dial), None);
        assert_eq!(grip.sample(1.0, -11.0, dial), Some(11.0));
        assert_eq!(grip.sample(20.0, -144.0, dial), Some(144.0));
        let mut rim = Grip::new(0.0, -34.0, 40.0);
        assert!((rim.sample(34.0, 0.0, dial).unwrap() - 72.0).abs() < 1e-4);
        assert!((rim.sample(-34.0, 0.0, dial).unwrap() - 72.0).abs() < 1e-4);
        assert!((rim.sample(0.0, 0.0, dial).unwrap() - 72.0).abs() < 1e-4);
        assert!((rim.sample(0.0, -30.0, dial).unwrap() - 72.0).abs() < 1e-4);
    }

    #[test]
    fn pulls_from_a_tangent_and_from_a_preset_keep_one_axis() {
        let dial = Drag::start(1.0, cover(1.7));
        let mut chord = Grip::new(-26., -30., 43.);
        assert_eq!(chord.sample(26., -30., dial), Some(52.));
        for (start, dir) in [
            ((0., -40.), (1., 0.)),
            ((0., 40.), (-1., 0.)),
            ((40., 0.), (0., -1.)),
            ((-40., 0.), (0., 1.)),
            ((-28., 27.), (1., 0.)),
            ((28., 27.), (-1., 0.)),
        ] {
            let mut g = Grip::new(start.0, start.1, 43.);
            for n in 1..=144 {
                let x = start.0 + dir.0 * n as f32;
                let y = start.1 + dir.1 * n as f32;
                if let Some(t) = g.sample(x, y, dial) {
                    assert!((t - (dir.0 - dir.1) * n as f32).abs() < 1e-3);
                }
            }
            assert!((g.travel.abs() - 144.).abs() < 1e-3);
        }
    }

    #[test]
    fn a_wandering_turn_keeps_its_sense_from_each_quadrant() {
        let dial = Drag::start(1.0, cover(1.73));
        for r in [25., 38., 48.] {
            for start in [15_f32, 105., 195., 285.] {
                for sign in [-1_f32, 1.] {
                    let point = |i: usize| {
                        let a = (start + sign * i as f32 * 3.).to_radians();
                        let radius = r * (1.0 + 0.095 * (2.0 * a).sin());
                        (radius * a.cos(), radius * a.sin())
                    };
                    let (x, y) = point(0);
                    let mut g = Grip::new(x, y, 43.);
                    let mut prev = 0.0;
                    for i in 1..=120 {
                        let (x, y) = point(i);
                        if let Some(t) = g.sample(x, y, dial) {
                            assert!(sign * (t - prev) >= -0.001);
                            prev = t;
                        }
                    }
                    assert!((g.travel - sign * 288.).abs() < 0.005);
                }
            }
        }
    }

    #[test]
    fn drag_is_monotonic_reversible_and_crosses_both_stops() {
        let dial = Drag::start(0.91, cover(1.72));
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
            assert!((Drag::start(value, cover(1.72)).at(0.0).value - value).abs() < 1e-6);
        }
    }

    #[test]
    fn wheel_passes_stops_in_both_directions_and_keeps_fast_input() {
        assert_eq!(step(0.96, cover(1.8), 1.12).stop, Stop::Page);
        assert!((step(1.0, cover(1.8), 1.12).value - 1.12).abs() < 1e-6);
        assert_eq!(step(1.7, cover(1.8), 1.12).stop, Stop::Fill);
        assert_eq!(step(1.9, cover(1.8), 1.0 / 1.12).stop, Stop::Fill);
        assert!(step(1.8, cover(1.8), 1.0 / 1.12).value < 1.8);
        assert_eq!(step(0.9, cover(1.8), 3.0).value, 0.9 * 3.0);
        assert_eq!(step(7.9, cover(1.8), 2.0).value, MAX);
        assert_eq!(step(0.26, cover(1.8), 0.5).value, MIN);
    }
}
