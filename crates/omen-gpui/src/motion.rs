use super::*;
use omen_core::{Animation, VisualState};
use std::time::Instant;
struct Motion {
    from: VisualState,
    target: VisualState,
    started: Instant,
    timing: Animation,
}
impl Motion {
    fn current(&self, now: Instant) -> VisualState {
        self.from.interpolate(
            self.target,
            self.timing
                .sample(now.saturating_duration_since(self.started)),
        )
    }
}
pub(crate) fn animate<T: gpui::Styled>(
    element: T,
    style: &Style,
    key: &SharedString,
    reduced: bool,
    window: &mut Window,
    cx: &mut App,
) -> T {
    let (x, y) = style.offset.unwrap_or_default();
    let target = VisualState {
        opacity: style.opacity.unwrap_or(1.),
        x,
        y,
    };
    if reduced || (style.animation.is_none() && style.transition.is_none()) {
        return apply(element, target);
    }
    let timing = style.animation.unwrap_or_default();
    let now = Instant::now();
    let state = window.use_keyed_state(SharedString::from(format!("{key}-motion")), cx, |_, _| {
        Motion {
            from: style
                .transition
                .map_or(target, |transition| target.entering(transition)),
            target,
            started: now,
            timing,
        }
    });
    let (visual, running) = state.update(cx, |state, _| {
        if state.target != target {
            state.from = state.current(now);
            state.target = target;
            state.started = now;
            state.timing = timing;
        }
        (
            state.current(now),
            now.saturating_duration_since(state.started) < state.timing.duration
                && state.from != state.target,
        )
    });
    if running {
        window.request_animation_frame();
    }
    apply(element, visual)
}
fn apply<T: gpui::Styled>(element: T, visual: VisualState) -> T {
    element
        .opacity(visual.opacity)
        .relative()
        .left(px(visual.x))
        .top(px(visual.y))
}
