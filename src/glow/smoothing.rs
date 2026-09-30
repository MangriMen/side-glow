use super::Rgb;

/// Largest time step applied at once. After an idle period the first frame would
/// otherwise have a huge `dt` and jump straight to the target.
const MAX_STEP_SECS: f64 = 1.0 / 30.0;
const SETTLED_EPSILON: f32 = 1.0 / 1024.0;

/// Frame-rate independent exponential smoothing of a row of colors.
#[derive(Default)]
pub struct Smoother {
    current: Vec<Rgb>,
    last_time: Option<f64>,
}

impl Smoother {
    /// Moves towards `target`; returns `true` while still animating.
    pub fn step(&mut self, target: &[Rgb], now: f64, smoothing_ms: f32) -> bool {
        if self.current.len() != target.len() {
            self.current = target.to_vec();
            self.last_time = Some(now);
            return false;
        }

        let dt = self
            .last_time
            .map_or(0.0, |last| (now - last).clamp(0.0, MAX_STEP_SECS)) as f32;
        self.last_time = Some(now);

        let tau = smoothing_ms / 1000.0;
        let alpha = if tau <= f32::EPSILON {
            1.0
        } else {
            1.0 - (-dt / tau).exp()
        };

        let mut animating = false;
        for (current, target) in self.current.iter_mut().zip(target) {
            for (c, t) in current.iter_mut().zip(target) {
                *c += (t - *c) * alpha;
                if (t - *c).abs() > SETTLED_EPSILON {
                    animating = true;
                } else {
                    *c = *t;
                }
            }
        }
        animating
    }

    pub fn colors(&self) -> &[Rgb] {
        &self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_independently_of_frame_rate() {
        let target = [[1.0, 0.5, 0.0]];
        let run = |fps: f64| {
            let mut s = Smoother::default();
            s.step(&[[0.0; 3]], 0.0, 200.0);
            let frames = (0.2 * fps).round() as u32;
            for i in 1..=frames {
                s.step(&target, f64::from(i) / fps, 200.0);
            }
            s.colors()[0][0]
        };
        // After one time constant both reach ~63 %, regardless of the frame rate.
        let (slow, fast) = (run(30.0), run(144.0));
        assert!((slow - fast).abs() < 0.03, "{slow} vs {fast}");
        assert!((fast - 0.632).abs() < 0.03, "{fast}");
    }

    #[test]
    fn zero_smoothing_snaps() {
        let mut s = Smoother::default();
        s.step(&[[0.0; 3]], 0.0, 0.0);
        assert!(!s.step(&[[1.0; 3]], 0.01, 0.0));
        assert_eq!(s.colors(), [[1.0; 3]]);
    }

    #[test]
    fn long_idle_does_not_jump() {
        let mut s = Smoother::default();
        s.step(&[[0.0; 3]], 0.0, 500.0);
        assert!(s.step(&[[1.0; 3]], 100.0, 500.0));
        assert!(s.colors()[0][0] < 0.1);
    }
}
