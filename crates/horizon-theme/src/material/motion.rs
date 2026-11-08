//! Material Expressive's motion springs.
//!
//! Spatial springs move position and shape; effects springs change colour and opacity. Integrate
//! with [`Spring::step`] in frame time.

/// Material's spring, integrated with semi-implicit Euler. `stiffness` and `damping` use the
/// same units as the Material motion scheme; `mass` is usually 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
    pub mass: f32,
}

impl Spring {
    pub const fn new(stiffness: f32, damping: f32, mass: f32) -> Self {
        Self {
            stiffness,
            damping,
            mass,
        }
    }

    /// A spring from Material's damping ratio: `damping = 2 * ratio * sqrt(stiffness * mass)`.
    pub fn from_ratio(stiffness: f32, ratio: f32) -> Self {
        let mass = 1.;
        Self {
            stiffness,
            damping: 2. * ratio * (stiffness * mass).sqrt(),
            mass,
        }
    }

    /// Advance one frame. The step is clamped and split so a long frame cannot explode.
    pub fn step(&self, state: &mut SpringState, target: f32, dt: f32) {
        const SUBSTEPS: usize = 4;
        let dt = dt.clamp(0., 1. / 30.);
        let h = dt / SUBSTEPS as f32;
        let inv_mass = if self.mass > 0. { 1. / self.mass } else { 0. };
        for _ in 0..SUBSTEPS {
            let force = -self.stiffness * (state.value - target) - self.damping * state.velocity;
            state.velocity += force * inv_mass * h;
            state.value += state.velocity * h;
        }
    }
}

/// The position and velocity of a [`Spring`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringState {
    pub value: f32,
    pub velocity: f32,
}

impl SpringState {
    pub const fn new(value: f32) -> Self {
        Self {
            value,
            velocity: 0.,
        }
    }

    pub fn settled(&self, target: f32) -> bool {
        (self.value - target).abs() <= 0.001 && self.velocity.abs() <= 0.001
    }
}

/// Material Expressive's motion springs. Spatial motion changes position and shape; effects
/// change colour and opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionScheme {
    pub spatial: Spring,
    pub spatial_fast: Spring,
    pub effects: Spring,
    pub effects_fast: Spring,
}

impl MotionScheme {
    pub fn expressive() -> Self {
        Self {
            spatial: Spring::from_ratio(380., 0.8),
            spatial_fast: Spring::from_ratio(800., 0.8),
            effects: Spring::from_ratio(700., 1.),
            effects_fast: Spring::from_ratio(1400., 1.),
        }
    }

    pub fn standard() -> Self {
        Self {
            spatial: Spring::from_ratio(300., 1.),
            spatial_fast: Spring::from_ratio(600., 1.),
            effects: Spring::from_ratio(500., 1.),
            effects_fast: Spring::from_ratio(1000., 1.),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_settles_on_the_target() {
        for spring in [
            MotionScheme::expressive().spatial,
            MotionScheme::expressive().effects,
            MotionScheme::standard().spatial,
        ] {
            let mut state = SpringState::new(0.);
            for _ in 0..240 {
                spring.step(&mut state, 1., 1. / 60.);
            }
            assert!(state.settled(1.), "state: {state:?}");
            assert!((state.value - 1.).abs() < 0.001);
        }
    }
}
