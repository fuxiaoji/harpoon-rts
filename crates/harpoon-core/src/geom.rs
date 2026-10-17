//! Flat-plane 2D geometry in nautical miles. +x = East, +y = North.
//!
//! Trig is implemented locally (range-reduced Taylor) instead of libm so that
//! simulations replay bit-identically across host platforms.

use serde::{Deserialize, Serialize};

pub const NM_PER_M: f64 = 1.0 / 1852.0;
pub const KTS_TO_NM_PER_S: f64 = 1.0 / 3600.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn norm(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Unit vector pointing along `bearing_deg` (0 = North, 90 = East).
    pub fn from_bearing(bearing_deg: f64) -> Self {
        let (s, c) = trig::sincos_deg(bearing_deg);
        Self { x: s, y: c }
    }

    pub fn scaled(&self, k: f64) -> Self {
        Self {
            x: self.x * k,
            y: self.y * k,
        }
    }

    pub fn add(&self, o: Vec2) -> Self {
        Self {
            x: self.x + o.x,
            y: self.y + o.y,
        }
    }

    pub fn sub(&self, o: Vec2) -> Self {
        Self {
            x: self.x - o.x,
            y: self.y - o.y,
        }
    }

    /// Distance in nautical miles.
    pub fn range_to(&self, to: Vec2) -> f64 {
        self.sub(to).norm()
    }

    /// Bearing from `self` to `to`, degrees in [0, 360).
    pub fn bearing_to(&self, to: Vec2) -> f64 {
        let d = to.sub(*self);
        if d.x.abs() < 1e-12 && d.y.abs() < 1e-12 {
            return 0.0;
        }
        let deg = d.y.atan2(d.x).to_degrees();
        normalize_deg(90.0 - deg)
    }

    /// Point `dist_nm` away from `self` along `bearing_deg`.
    pub fn advanced(&self, bearing_deg: f64, dist_nm: f64) -> Vec2 {
        let d = Vec2::from_bearing(bearing_deg).scaled(dist_nm);
        self.add(d)
    }
}

/// Normalize an angle to [0, 360).
pub fn normalize_deg(deg: f64) -> f64 {
    let mut d = deg % 360.0;
    if d < 0.0 {
        d += 360.0;
    }
    d
}

/// Signed shortest difference `to - from` in (-180, 180].
pub fn angle_diff(from_deg: f64, to_deg: f64) -> f64 {
    let d = (to_deg - from_deg + 180.0) % 360.0 - 180.0;
    if d > 180.0 {
        d - 360.0
    } else if d <= -180.0 {
        d + 360.0
    } else {
        d
    }
}

/// Radar/visual horizon between two heights given in meters:
/// `1.23 * (sqrt(h1_ft) + sqrt(h2_ft))` nautical miles (Harpoon V rules
/// §5.2.8 radar horizon table regressed to closed form).
pub fn horizon_nm(alt_m_a: f64, alt_m_b: f64) -> f64 {
    let ft_a = alt_m_a.max(0.0) * 3.28084;
    let ft_b = alt_m_b.max(0.0) * 3.28084;
    1.23 * (ft_a.sqrt() + ft_b.sqrt())
}

/// Deterministic cross-platform trigonometry.
pub mod trig {
    const PI: f64 = std::f64::consts::PI;

    fn reduce(x_deg: f64) -> (f64, bool, bool) {
        let mut x = x_deg % 360.0;
        if x < 0.0 {
            x += 360.0;
        }
        let neg = x >= 180.0;
        if neg {
            x -= 180.0;
        }
        let flip = x >= 90.0;
        if flip {
            x = 180.0 - x;
        }
        (x, neg, flip)
    }

    /// Taylor sine on [0, pi/2], adequate to ~1e-12 for wargame use.
    fn sin_core(x: f64) -> f64 {
        let r = x * PI / 180.0;
        let r2 = r * r;
        let mut term = r;
        let mut sum = r;
        for k in 1..12 {
            term *= -r2 / (((2 * k) * (2 * k + 1)) as f64);
            sum += term;
            if term.abs() < 1e-16 {
                break;
            }
        }
        sum
    }

    fn cos_core(x: f64) -> f64 {
        sin_core(90.0 - x)
    }

    /// (sin, cos) of an angle in degrees.
    pub fn sincos_deg(deg: f64) -> (f64, f64) {
        let (x, neg, flip) = reduce(deg);
        let s = sin_core(x);
        let c = cos_core(x);
        match (neg, flip) {
            (false, false) => (s, c),   // a = x
            (false, true) => (s, -c),   // a = 180 - x
            (true, false) => (-s, -c),  // a = 180 + x
            (true, true) => (-s, c),    // a = 360 - x
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn bearings() {
        let a = Vec2::new(0.0, 0.0);
        assert!(close(a.bearing_to(Vec2::new(10.0, 0.0)), 90.0, 1e-6));
        assert!(close(a.bearing_to(Vec2::new(0.0, 10.0)), 0.0, 1e-6));
        assert!(close(a.bearing_to(Vec2::new(-10.0, 0.0)), 270.0, 1e-6));
        assert!(close(angle_diff(350.0, 10.0), 20.0, 1e-9));
        assert!(close(angle_diff(10.0, 350.0), -20.0, 1e-9));
    }

    #[test]
    fn trig_matches_libm_closely() {
        for deg in [0.0, 17.3, 45.0, 89.9, 133.0, 271.5, 359.99] {
            let (s, c) = trig::sincos_deg(deg);
            assert!(close(s, deg.to_radians().sin(), 1e-9), "sin {deg}");
            assert!(close(c, deg.to_radians().cos(), 1e-9), "cos {deg}");
        }
    }

    #[test]
    fn horizon() {
        // 30 m radar vs 8 m sea-skimmer: 1.23*(sqrt(98)+sqrt(26)) ≈ 18.5 nm
        assert!(horizon_nm(30.0, 8.0) > 17.0 && horizon_nm(30.0, 8.0) < 20.0);
        // Table anchor: 100 ft + 100 ft → 1.23*20 = 24.6 nm
        assert!((horizon_nm(30.48, 30.48) - 24.6).abs() < 0.3);
        // High radar vs high aircraft: far beyond 100 nm
        assert!(horizon_nm(30.0, 9000.0) > 120.0);
    }
}
