//! A route's polyline by arc length: where along it a boat is, how far off it, and the point some way ahead.

/// Points with their cumulative distance from the first.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Polyline {
    points: Vec<[f32; 2]>,
    cum: Vec<f32>,
}

/// Where a point lies against the line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Place {
    /// Distance along the line of the nearest point on it.
    pub s: f32,
    /// Distance from the line.
    pub off: f32,
    /// Segment holding the nearest point.
    pub seg: usize,
}

impl Polyline {
    pub fn new(points: Vec<[f32; 2]>) -> Self {
        let mut cum = Vec::with_capacity(points.len());
        let mut total = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                let q = points[i - 1];
                total += (p[0] - q[0]).hypot(p[1] - q[1]);
            }
            cum.push(total);
        }
        Self { points, cum }
    }

    pub fn length(&self) -> f32 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    pub fn end(&self) -> [f32; 2] {
        self.points.last().copied().unwrap_or_default()
    }

    /// The nearest place on segments `from_seg..` within `window` segments: progress never jumps back across a
    /// bend, nor ahead to a later leg passing close by.
    pub fn locate(&self, p: [f32; 2], from_seg: usize, window: usize) -> Place {
        let last = self.points.len().saturating_sub(1);
        if last == 0 {
            return Place { s: 0.0, off: dist(p, self.end()), seg: 0 };
        }
        let mut best = Place { s: 0.0, off: f32::MAX, seg: from_seg.min(last - 1) };
        for seg in from_seg.min(last - 1)..(from_seg + window).min(last) {
            let (a, b) = (self.points[seg], self.points[seg + 1]);
            let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
            let len2 = dx * dx + dz * dz;
            let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let off = dist(p, [a[0] + dx * t, a[1] + dz * t]);
            if off < best.off {
                best = Place { s: self.cum[seg] + t * len2.sqrt(), off, seg };
            }
        }
        best
    }

    /// The point `s` along the line, clamped to its ends.
    pub fn at(&self, s: f32) -> [f32; 2] {
        let s = s.clamp(0.0, self.length());
        let seg = self.cum.partition_point(|&c| c <= s).clamp(1, self.points.len().max(2) - 1);
        let (a, b) = (self.points[seg - 1], self.points.get(seg).copied().unwrap_or(self.points[seg - 1]));
        let len = self.cum.get(seg).map_or(0.0, |c| c - self.cum[seg - 1]);
        let t = if len > 0.0 { (s - self.cum[seg - 1]) / len } else { 0.0 };
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }
}

pub(crate) fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
