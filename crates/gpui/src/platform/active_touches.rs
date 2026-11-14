use crate::{Pixels, Point, TouchEvent, TouchId, TouchPhase};
use collections::HashMap;
use std::hash::Hash;

struct ActiveTouch<W> {
    id: TouchId,
    window: W,
    position: Point<Pixels>,
}

/// Gives platform touches never-reused ids and keeps each touch with the window it started in.
pub struct ActiveTouches<K, W> {
    next_id: u64,
    by_slot: HashMap<K, ActiveTouch<W>>,
}

impl<K, W> Default for ActiveTouches<K, W> {
    fn default() -> Self {
        Self {
            next_id: 0,
            by_slot: HashMap::default(),
        }
    }
}

impl<K: Hash + Eq, W: Clone> ActiveTouches<K, W> {
    /// Records a new touch, cancelling any touch still using the same platform number.
    pub fn start(&mut self, slot: K, window: W, position: Point<Pixels>) -> Vec<(W, TouchEvent)> {
        let mut events = Vec::new();
        if let Some(stale) = self.by_slot.remove(&slot) {
            events.push((
                stale.window,
                event(stale.id, TouchPhase::Cancelled, stale.position),
            ));
        }
        let id = TouchId(self.next_id);
        self.next_id += 1;
        self.by_slot.insert(
            slot,
            ActiveTouch {
                id,
                window: window.clone(),
                position,
            },
        );
        events.push((window, event(id, TouchPhase::Started, position)));
        events
    }

    /// Reports a touch that moved, or `None` if it is unknown or did not move.
    pub fn moved(&mut self, slot: &K, position: Point<Pixels>) -> Option<(W, TouchEvent)> {
        let touch = self.by_slot.get_mut(slot)?;
        if touch.position == position {
            return None;
        }
        touch.position = position;
        Some((
            touch.window.clone(),
            event(touch.id, TouchPhase::Moved, position),
        ))
    }

    /// Reports a touch that lifted, at its last known position.
    pub fn ended(&mut self, slot: &K) -> Option<(W, TouchEvent)> {
        let touch = self.by_slot.remove(slot)?;
        Some((
            touch.window,
            event(touch.id, TouchPhase::Ended, touch.position),
        ))
    }

    /// Reports a touch the platform took back, at its last known position.
    pub fn cancelled(&mut self, slot: &K) -> Option<(W, TouchEvent)> {
        let touch = self.by_slot.remove(slot)?;
        Some((
            touch.window,
            event(touch.id, TouchPhase::Cancelled, touch.position),
        ))
    }

    /// Forgets the touches that started in a window that is going away.
    pub fn forget_window(&mut self, mut is_window: impl FnMut(&W) -> bool) {
        self.by_slot.retain(|_, touch| !is_window(&touch.window));
    }

    /// Cancels every touch, for platforms that cancel all touches at once.
    pub fn cancel_all(&mut self) -> Vec<(W, TouchEvent)> {
        let mut touches: Vec<_> = self.by_slot.drain().map(|(_, touch)| touch).collect();
        touches.sort_by_key(|touch| touch.id);
        touches
            .into_iter()
            .map(|touch| {
                (
                    touch.window,
                    event(touch.id, TouchPhase::Cancelled, touch.position),
                )
            })
            .collect()
    }
}

fn event(id: TouchId, phase: TouchPhase, position: Point<Pixels>) -> TouchEvent {
    TouchEvent {
        id,
        phase,
        position,
        predicted_position: None,
        force: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{point, px};

    fn summary(events: Vec<(&'static str, TouchEvent)>) -> Vec<(&'static str, u64, TouchPhase)> {
        events
            .into_iter()
            .map(|(window, event)| (window, event.id.0, event.phase))
            .collect()
    }

    #[test]
    fn reused_slots_get_fresh_ids() {
        let mut touches = ActiveTouches::<i32, &str>::default();
        let first = touches.start(0, "editor", point(px(1.), px(1.)));
        touches.ended(&0);
        let second = touches.start(0, "editor", point(px(1.), px(1.)));
        assert_eq!(summary(first), vec![("editor", 0, TouchPhase::Started)]);
        assert_eq!(summary(second), vec![("editor", 1, TouchPhase::Started)]);
    }

    #[test]
    fn a_touch_stays_with_the_window_it_started_in() {
        let mut touches = ActiveTouches::<i32, &str>::default();
        touches.start(3, "main", point(px(10.), px(20.)));

        let (window, moved) = touches.moved(&3, point(px(15.), px(40.))).unwrap();
        assert_eq!(window, "main");
        assert_eq!(moved.phase, TouchPhase::Moved);

        let (window, ended) = touches.ended(&3).unwrap();
        assert_eq!(window, "main");
        assert_eq!(ended.phase, TouchPhase::Ended);
        assert_eq!(ended.position, point(px(15.), px(40.)));

        assert!(touches.moved(&3, point(px(0.), px(0.))).is_none());
        assert!(touches.ended(&3).is_none());
    }

    #[test]
    fn unchanged_positions_are_not_reported_as_moves() {
        let mut touches = ActiveTouches::<i32, &str>::default();
        touches.start(0, "main", point(px(5.), px(5.)));
        assert!(touches.moved(&0, point(px(5.), px(5.))).is_none());
    }

    #[test]
    fn forgetting_a_window_drops_only_its_touches() {
        let mut touches = ActiveTouches::<i32, &str>::default();
        touches.start(0, "main", point(px(1.), px(1.)));
        touches.start(1, "popup", point(px(2.), px(2.)));
        touches.forget_window(|window| *window == "popup");
        assert!(touches.ended(&1).is_none());
        assert_eq!(
            summary(touches.ended(&0).into_iter().collect()),
            vec![("main", 0, TouchPhase::Ended)]
        );
    }

    #[test]
    fn cancel_ends_every_touch() {
        let mut touches = ActiveTouches::<i32, &str>::default();
        touches.start(0, "main", point(px(1.), px(1.)));
        touches.start(1, "popup", point(px(2.), px(2.)));
        assert_eq!(
            summary(touches.cancel_all()),
            vec![
                ("main", 0, TouchPhase::Cancelled),
                ("popup", 1, TouchPhase::Cancelled),
            ]
        );
        assert!(touches.ended(&0).is_none());
    }

    #[test]
    fn a_cancelled_touch_is_forgotten() {
        let mut touches = ActiveTouches::<i32, &str>::default();
        touches.start(0, "main", point(px(1.), px(1.)));
        let (_, cancelled) = touches.cancelled(&0).unwrap();
        assert_eq!(cancelled.phase, TouchPhase::Cancelled);
        assert!(touches.ended(&0).is_none());
    }

    #[test]
    fn a_slot_reused_without_an_up_cancels_the_old_touch() {
        let mut touches = ActiveTouches::<i32, &str>::default();
        touches.start(0, "main", point(px(1.), px(1.)));
        assert_eq!(
            summary(touches.start(0, "main", point(px(9.), px(9.)))),
            vec![
                ("main", 0, TouchPhase::Cancelled),
                ("main", 1, TouchPhase::Started),
            ]
        );
    }
}
