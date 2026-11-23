//! Host lifecycle and portable gesture semantics.
/// Whether the host is displaying application content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppLifecycle {
    /// Content is visible and accepts interaction.
    #[default]
    Foreground,
    /// Content is hidden. Mounted resources are suspended; local state is retained.
    Background,
}
/// Physical swipe direction, independent of text direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeDirection {
    /// Movement toward the left.
    Left,
    /// Movement toward the right.
    Right,
    /// Movement upward.
    Up,
    /// Movement downward.
    Down,
}
/// Thresholds used by pointer-based gesture recognizers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GesturePolicy {
    /// Maximum displacement for a long press, in CSS/logical pixels.
    pub slop: f32,
    /// Minimum swipe displacement.
    pub swipe_distance: f32,
    /// Long-press duration in milliseconds.
    pub long_press_ms: u32,
}
impl Default for GesturePolicy {
    fn default() -> Self {
        Self {
            slop: 10.,
            swipe_distance: 40.,
            long_press_ms: 500,
        }
    }
}
impl GesturePolicy {
    /// Classify a completed pointer gesture. Cancellation must be handled by the host.
    pub fn classify(self, dx: f32, dy: f32, elapsed_ms: u32) -> Option<SwipeDirection> {
        if elapsed_ms > 1200 || dx.abs().max(dy.abs()) < self.swipe_distance {
            return None;
        }
        Some(if dx.abs() > dy.abs() {
            if dx < 0. {
                SwipeDirection::Left
            } else {
                SwipeDirection::Right
            }
        } else if dy < 0. {
            SwipeDirection::Up
        } else {
            SwipeDirection::Down
        })
    }
}
