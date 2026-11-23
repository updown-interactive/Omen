//! Deterministic easing and damped spring sampling.
use std::time::Duration;
/// Easing curves for timed animations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Easing {
    /// Constant velocity.
    Linear,
    /// Accelerate from rest.
    EaseIn,
    /// Decelerate to rest.
    EaseOut,
    /// Smooth acceleration and deceleration.
    EaseInOut,
}
impl Easing {
    /// Sample normalized time, clamping outside zero through one.
    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0., 1.);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1. - (1. - t) * (1. - t),
            Self::EaseInOut => t * t * (3. - 2. * t),
        }
    }
}
/// Damped spring with mass one. Inputs are sanitized when sampled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Spring stiffness; must be positive.
    pub stiffness: f32,
    /// Damping coefficient; must be positive.
    pub damping: f32,
}
impl Default for Spring {
    fn default() -> Self {
        Self {
            stiffness: 180.,
            damping: 22.,
        }
    }
}
impl Spring {
    /// Step response starting at zero with zero initial velocity.
    pub fn sample(self, seconds: f32) -> f32 {
        let k = if self.stiffness.is_finite() {
            self.stiffness.max(0.01)
        } else {
            180.
        };
        let c = if self.damping.is_finite() {
            self.damping.max(0.01)
        } else {
            22.
        };
        let t = seconds.max(0.);
        let w = k.sqrt();
        let z = c / (2. * w);
        if z < 1. - 0.0001 {
            let wd = w * (1. - z * z).sqrt();
            1. - (-z * w * t).exp() * ((wd * t).cos() + z * w / wd * (wd * t).sin())
        } else if z > 1. + 0.0001 {
            let root = (z * z - 1.).sqrt();
            let a = -w * (z - root);
            let b = -w * (z + root);
            1. - (b * (a * t).exp() - a * (b * t).exp()) / (b - a)
        } else {
            1. - (-w * t).exp() * (1. + w * t)
        }
    }
}
/// Animation timing, shared by style interpolation and entry transitions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    /// Time after which the target is exact.
    pub duration: Duration,
    /// Easing for a timed animation.
    pub easing: Easing,
    /// Optional spring response instead of easing.
    pub spring: Option<Spring>,
}
impl Animation {
    /// A timed animation.
    pub fn new(duration: Duration, easing: Easing) -> Self {
        Self {
            duration,
            easing,
            spring: None,
        }
    }
    /// A spring animation with a finite settling deadline.
    pub fn spring(duration: Duration, spring: Spring) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
            spring: Some(spring),
        }
    }
    /// Sample elapsed time. Zero duration and completed animations resolve immediately.
    pub fn sample(self, elapsed: Duration) -> f32 {
        if self.duration.is_zero() || elapsed >= self.duration {
            1.
        } else if let Some(spring) = self.spring {
            spring.sample(elapsed.as_secs_f32())
        } else {
            self.easing
                .sample(elapsed.as_secs_f32() / self.duration.as_secs_f32())
        }
    }
}
impl Default for Animation {
    fn default() -> Self {
        Self::new(Duration::from_millis(220), Easing::EaseInOut)
    }
}
/// Entry transition for a newly mounted native node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    /// Fade from transparent.
    Fade,
    /// Fade while moving upward by sixteen pixels.
    Slide,
}
/// Interpolated visual properties, independent of layout measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualState {
    /// Opacity, zero through one.
    pub opacity: f32,
    /// Horizontal translation.
    pub x: f32,
    /// Vertical translation.
    pub y: f32,
}
impl Default for VisualState {
    fn default() -> Self {
        Self {
            opacity: 1.,
            x: 0.,
            y: 0.,
        }
    }
}
impl VisualState {
    /// Interpolate toward a target. Spring progress may overshoot.
    pub fn interpolate(self, target: Self, t: f32) -> Self {
        Self {
            opacity: (self.opacity + (target.opacity - self.opacity) * t).clamp(0., 1.),
            x: self.x + (target.x - self.x) * t,
            y: self.y + (target.y - self.y) * t,
        }
    }
    /// Initial state for an entry transition toward this target.
    pub fn entering(self, transition: Transition) -> Self {
        match transition {
            Transition::Fade => Self {
                opacity: 0.,
                ..self
            },
            Transition::Slide => Self {
                opacity: 0.,
                y: self.y + 16.,
                ..self
            },
        }
    }
}
