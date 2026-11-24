//! A trackpad's edge stretch on macOS, as `NSScrollView` does it: a phased gesture's fingers pull
//! a vertical viewport past its ends on a rubber band (`rune_motion::rubber_band`), the
//! fingers lifting from a stretched viewport spring it back, and momentum that reaches an end
//! carries on past it at the speed it had and springs back. Either way the rest of that gesture's
//! momentum is dropped. A notched wheel, and every platform but macOS, stop at the ends
//! (`ScrollTables::elastic`). Reduced motion keeps it: it is the hand's own motion.
use super::touch::rebound;
use crate::machine::Effects;

/// Fastest a bounce starts, in points a second, as a touch fling's speed is capped.
const MOST_SPEED: f64 = 8000.0;
/// Seconds between momentum deltas when the last delta is unknown: one 60 Hz frame.
const FRAME: f64 = 1.0 / 60.0;

/// What a phased gesture has done to one viewport's ends.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::runtime) enum Stretch {
    /// Nothing: the viewport stops at its ends.
    #[default]
    Free,
    /// The gesture's deltas move it, the last at `at`; the fingers may hold it past an end.
    Held { at: f64 },
    /// Springing back to its end at `v` points a second (offset per second).
    Returning { v: f32 },
    /// Back at its end: the rest of the gesture's momentum is dropped, until fingers move it.
    Spent,
}

/// A viewport's vertical offset, its end and its height, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) struct Axis {
    pub(in crate::runtime) offset: f64,
    pub(in crate::runtime) max: f64,
    pub(in crate::runtime) height: f64,
}

impl Axis {
    /// The end nearest the offset, or the offset itself between them.
    fn edge(self) -> f64 { self.offset.clamp(0.0, self.max) }

    fn past(self) -> bool { self.offset != self.edge() }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) enum Event {
    /// The fingers moved content `dy` points (positive down) at `now`.
    Finger { dy: f64, now: f64, axis: Axis },
    /// The gesture's momentum moved content `dy` points at `now`.
    Momentum { dy: f64, now: f64, axis: Axis },
    /// A frame `dt` seconds on; `touching`: the fingers are still on the trackpad.
    Tick { dt: f32, touching: bool, axis: Axis },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::runtime) enum Effect {
    /// The hand put the offset here.
    Set(f64),
    /// The fingers hold it past an end this frame.
    Hold,
    /// Springing back, it is at `offset`, still `moving` or come to rest.
    Spring { offset: f64, moving: bool },
}

/// Folds `event` into a viewport's `state`. A tick with no effect leaves the frame to the
/// viewport's own motion.
pub(in crate::runtime) fn step(state: &mut Stretch, event: Event) -> Effects<Effect> {
    match (*state, event) {
        (_, Event::Finger { dy, now, axis }) => {
            *state = Stretch::Held { at: now };
            Effects::one(Effect::Set(pull(axis, dy)))
        }
        (Stretch::Returning { .. } | Stretch::Spent, Event::Momentum { .. }) => Effects::none(),
        (_, Event::Momentum { axis, .. }) if axis.past() => {
            *state = Stretch::Returning { v: 0.0 };
            Effects::none()
        }
        (last, Event::Momentum { dy, now, axis }) => {
            let to = axis.offset - dy;
            let edge = to.clamp(0.0, axis.max);
            if to == edge {
                *state = Stretch::Held { at: now };
                return Effects::one(Effect::Set(to));
            }
            let since = match last { Stretch::Held { at } => (now - at).clamp(FRAME / 2.0, FRAME * 3.0), _ => FRAME };
            let v = (-dy / since).clamp(-MOST_SPEED, MOST_SPEED);
            *state = Stretch::Returning { v: v as f32 };
            Effects::one(Effect::Set(edge))
        }
        (Stretch::Held { .. }, Event::Tick { touching: true, axis, .. }) if axis.past() => Effects::one(Effect::Hold),
        (Stretch::Held { .. }, Event::Tick { dt, axis, .. }) if axis.past() => spring_back(state, axis, 0.0, dt),
        (Stretch::Returning { v }, Event::Tick { dt, axis, .. }) => spring_back(state, axis, v, dt),
        (_, Event::Tick { .. }) => Effects::none(),
    }
}

/// Where the fingers moving content `dy` points put the offset: the band past an end is undone
/// to the distance the fingers travelled, moved on, and banded again, so pulling back retraces it.
fn pull(axis: Axis, dy: f64) -> f64 {
    let edge = axis.edge();
    let raw = edge + unband(axis.offset - edge, axis.height) - dy;
    let to = raw.clamp(0.0, axis.max);
    to + f64::from(rune_motion::rubber_band((raw - to) as f32, axis.height as f32))
}

/// The distance past an end that `rubber_band` draws as `banded` on a viewport `height` tall.
fn unband(banded: f64, height: f64) -> f64 {
    if height <= 0.0 || banded == 0.0 { return 0.0; }
    let y = banded.abs().min(height * 0.999);
    (height / 0.55 * y / (height - y)).copysign(banded)
}

/// One frame of the spring back to the end: critically damped, as a touch's edge return.
fn spring_back(state: &mut Stretch, axis: Axis, v: f32, dt: f32) -> Effects<Effect> {
    let end = axis.edge();
    let (offset, v) = rebound(axis.offset, v, end, dt);
    let landed = (offset - end).abs() < 0.1 && v.abs() < 1.0;
    *state = if landed { Stretch::Spent } else { Stretch::Returning { v } };
    Effects::one(Effect::Spring { offset: if landed { end } else { offset }, moving: !landed })
}

#[cfg(test)]
#[path = "stretch_tests.rs"]
mod tests;
