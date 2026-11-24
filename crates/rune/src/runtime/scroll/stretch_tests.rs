use super::*;
use crate::runtime::Runtime;
use crate::{Cx, El, Env, GesturePhase, WheelEvent};

const DT: f64 = 1.0 / 60.0;

fn list(_: &(), _: &mut Cx<()>) -> El {
    let rows = (0..20).fold(crate::col().w_full(), |c, i| c.child(crate::el().key(format!("row{i}")).w_full().h(40.0)));
    crate::col().w_full().h_full().child(crate::col().key("list").w_full().h(300.0)
        .scroll(crate::ScrollSpec { stick_bottom: false }).child(rows))
}

fn runtime(elastic: bool) -> Runtime<()> {
    let mut rt = Runtime::new((), list, (300.0, 300.0), Env::default());
    rt.scroll.set_elastic(elastic);
    rt.frame(0.0);
    rt.frame(DT);
    rt
}

fn offset(rt: &Runtime<()>) -> f64 {
    let id = rt.tree.nodes.iter().find(|n| n.key.as_deref() == Some("list")).unwrap().id;
    rt.scroll.scrolls[&id].offset
}

/// Fingers pull content down at the top: each delta a frame.
fn pull(rt: &mut Runtime<()>, dy: f32, deltas: usize) {
    rt.wheel_event(WheelEvent::new(0.0, 0.0).at(150.0, 150.0).phase(GesturePhase::Began));
    for _ in 0..deltas {
        rt.wheel_event(WheelEvent::new(0.0, dy).at(150.0, 150.0).precise(true));
        rt.frame(DT);
    }
}

#[test]
fn trackpad_fingers_stretch_past_the_top_hold_it_and_spring_back_on_lift() {
    let mut rt = runtime(true);
    pull(&mut rt, 20.0, 5);
    let held = offset(&rt);
    // 100 points of travel on a 300-point viewport: 300 * (1 - 1 / (100 * 0.55 / 300 + 1)).
    assert!((held + 46.48).abs() < 0.1, "stretched on the band, not 1:1 and not clamped: {held}");
    for _ in 0..10 { rt.frame(DT); }
    assert_eq!(offset(&rt), held, "the fingers still down hold it there");
    rt.wheel_event(WheelEvent::new(0.0, -20.0).at(150.0, 150.0).precise(true));
    assert!(offset(&rt) > held && offset(&rt) < 0.0, "pulling back retraces the band: {}", offset(&rt));
    rt.wheel_event(WheelEvent::new(0.0, 0.0).at(150.0, 150.0).phase(GesturePhase::Ended));
    // Its momentum begins at once and is dropped: the lift springs it back regardless.
    rt.wheel_event(WheelEvent::new(0.0, 0.0).at(150.0, 150.0).phase(GesturePhase::Began));
    rt.wheel_event(WheelEvent::new(0.0, 15.0).at(150.0, 150.0).precise(true));
    rt.frame(DT);
    let first = offset(&rt);
    assert!(first < 0.0 && first > held, "springs back from where it was: {first}");
    for _ in 0..60 {
        rt.wheel_event(WheelEvent::new(0.0, 5.0).at(150.0, 150.0).precise(true));
        rt.frame(DT);
    }
    assert_eq!(offset(&rt), 0.0, "and lands on the end");
}

#[test]
fn momentum_that_reaches_an_end_bounces_past_it_and_back() {
    let mut rt = runtime(true);
    pull(&mut rt, -30.0, 3);
    assert_eq!(offset(&rt), 90.0, "inside the content, fingers move it 1:1");
    rt.wheel_event(WheelEvent::new(0.0, 0.0).at(150.0, 150.0).phase(GesturePhase::Ended));
    rt.wheel_event(WheelEvent::new(0.0, 0.0).at(150.0, 150.0).phase(GesturePhase::Began));
    for _ in 0..6 {
        rt.wheel_event(WheelEvent::new(0.0, 30.0).at(150.0, 150.0).precise(true));
        rt.frame(DT);
    }
    let mut least = 0.0f64;
    for _ in 0..90 { rt.frame(DT); least = least.min(offset(&rt)); }
    assert!(least < -5.0, "carried past the top: {least}");
    assert_eq!(offset(&rt), 0.0, "and back to it");
}

#[test]
fn a_notched_wheel_or_an_unphased_trackpad_stops_at_the_end() {
    let mut rt = runtime(true);
    rt.wheel_event(WheelEvent::new(0.0, 40.0).at(150.0, 150.0).notch(true));
    rt.wheel_event(WheelEvent::new(0.0, 7.5).at(150.0, 150.0).precise(true));
    rt.frame(DT);
    assert_eq!(offset(&rt), 0.0);
    let mut off = runtime(false);
    pull(&mut off, 20.0, 5);
    assert_eq!(offset(&off), 0.0, "off macOS a phased trackpad stops too");
}

#[test]
fn content_that_fits_stretches_too() {
    fn short(_: &(), _: &mut Cx<()>) -> El {
        crate::col().w_full().h_full().child(crate::col().key("list").w_full().h(300.0)
            .scroll(crate::ScrollSpec { stick_bottom: false }).child(crate::el().w_full().h(100.0)))
    }
    let mut rt = Runtime::new((), short, (300.0, 300.0), Env::default());
    rt.scroll.set_elastic(true);
    rt.frame(0.0);
    rt.frame(DT);
    pull(&mut rt, -20.0, 5);
    assert!(offset(&rt) > 40.0, "pulled up past its bottom: {}", offset(&rt));
}

#[test]
fn the_band_undone_is_the_distance_travelled() {
    for over in [-250.0f32, -10.0, 3.0, 120.0] {
        let banded = rune_motion::rubber_band(over, 300.0);
        assert!((unband(f64::from(banded), 300.0) - f64::from(over)).abs() < 0.01, "{over}");
    }
}
