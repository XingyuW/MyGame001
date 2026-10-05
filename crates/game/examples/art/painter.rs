//! Small deterministic pixel-art primitives, shared by the repository's original art generators.
pub type Rgb = [u8; 3];
pub const STEEL: Rgb = [126, 139, 142];
pub const DARK: Rgb = [39, 49, 52];
pub const TRIM: Rgb = [193, 187, 147];
pub const TEAM: Rgb = [42, 133, 202];
pub const LAMP: Rgb = [166, 219, 201];
pub fn shade(c: Rgb, delta: i16) -> Rgb {
    c.map(|v| (i16::from(v) + delta).clamp(0, 255) as u8)
}
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub origin: (f64, f64),
}
impl Canvas {
    pub fn new(width: u32, height: u32, origin: (f64, f64)) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
            origin,
        }
    }
    pub fn dot(&mut self, x: i32, y: i32, c: Rgb) {
        if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
            let i = ((y as u32 * self.width + x as u32) * 4) as usize;
            self.pixels[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    pub fn point(&self, p: (f64, f64, f64)) -> (f64, f64) {
        (
            self.origin.0 + (p.0 - p.1) * 32.0,
            self.origin.1 + (p.0 + p.1) * 16.0 - p.2,
        )
    }
    pub fn line(&mut self, a: (f64, f64), b: (f64, f64), c: Rgb) {
        let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil() as i32).max(1);
        for i in 0..=steps {
            let t = f64::from(i) / f64::from(steps);
            self.dot(
                (a.0 + (b.0 - a.0) * t).round() as i32,
                (a.1 + (b.1 - a.1) * t).round() as i32,
                c,
            );
        }
    }
    pub fn beam(&mut self, a: (f64, f64, f64), b: (f64, f64, f64), c: Rgb) {
        self.line(self.point(a), self.point(b), c);
    }
    pub fn polygon(&mut self, points: &[(f64, f64)], c: Rgb) {
        let lo_x = points
            .iter()
            .map(|p| p.0.floor() as i32)
            .min()
            .unwrap_or(0)
            .max(0);
        let hi_x = points
            .iter()
            .map(|p| p.0.ceil() as i32)
            .max()
            .unwrap_or(0)
            .min(self.width as i32 - 1);
        let lo_y = points
            .iter()
            .map(|p| p.1.floor() as i32)
            .min()
            .unwrap_or(0)
            .max(0);
        let hi_y = points
            .iter()
            .map(|p| p.1.ceil() as i32)
            .max()
            .unwrap_or(0)
            .min(self.height as i32 - 1);
        for y in lo_y..=hi_y {
            for x in lo_x..=hi_x {
                let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                let mut inside = false;
                let mut j = points.len() - 1;
                for i in 0..points.len() {
                    let (a, b) = (points[i], points[j]);
                    if (a.1 > py) != (b.1 > py) && px < (b.0 - a.0) * (py - a.1) / (b.1 - a.1) + a.0
                    {
                        inside = !inside;
                    }
                    j = i;
                }
                if inside {
                    // Restrained two-tone material grain, fixed by pixel coordinate (no RNG).
                    let n = (x as u32).wrapping_mul(17) ^ (y as u32).wrapping_mul(37);
                    self.dot(x, y, shade(c, (n % 7) as i16 - 3));
                }
            }
        }
    }
    pub fn face(&mut self, points: &[(f64, f64, f64)], c: Rgb) {
        let projected: Vec<_> = points.iter().map(|&p| self.point(p)).collect();
        self.polygon(&projected, c);
    }
    pub fn block(&mut self, p: (f64, f64, f64), size: (f64, f64, f64), c: Rgb) {
        let (u, v, z) = p;
        let (w, d, h) = size;
        self.face(
            &[
                (u, v + d, z),
                (u + w, v + d, z),
                (u + w, v + d, z + h),
                (u, v + d, z + h),
            ],
            shade(c, -18),
        );
        self.face(
            &[
                (u + w, v, z),
                (u + w, v + d, z),
                (u + w, v + d, z + h),
                (u + w, v, z + h),
            ],
            shade(c, -44),
        );
        self.face(
            &[
                (u, v, z + h),
                (u + w, v, z + h),
                (u + w, v + d, z + h),
                (u, v + d, z + h),
            ],
            shade(c, 22),
        );
        self.beam((u, v + d, z + h), (u + w, v + d, z + h), shade(c, 45));
        self.beam((u + w, v, z + h), (u + w, v + d, z + h), shade(c, 4));
    }
    pub fn ellipse(&mut self, centre: (f64, f64), radii: (f64, f64), c: Rgb) {
        for y in (centre.1 - radii.1).floor() as i32..=(centre.1 + radii.1).ceil() as i32 {
            for x in (centre.0 - radii.0).floor() as i32..=(centre.0 + radii.0).ceil() as i32 {
                let dx = (f64::from(x) + 0.5 - centre.0) / radii.0;
                let dy = (f64::from(y) + 0.5 - centre.1) / radii.1;
                if dx * dx + dy * dy <= 1.0 {
                    self.dot(x, y, c);
                }
            }
        }
    }
    pub fn cylinder(&mut self, p: (f64, f64, f64), radius: f64, height: f64, c: Rgb) {
        let (cx, cy) = self.point(p);
        let rx = radius * 38.0;
        let ry = radius * 19.0;
        for y in (cy - height).floor() as i32..=cy.ceil() as i32 {
            for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
                let nx = (f64::from(x) + 0.5 - cx) / rx;
                if nx.abs() <= 1.0 {
                    self.dot(x, y, shade(c, (-nx * 28.0 - 10.0) as i16));
                }
            }
        }
        self.ellipse((cx, cy), (rx, ry), shade(c, -18));
        self.ellipse((cx, cy - height), (rx, ry), shade(c, 32));
        self.ellipse((cx, cy - height), (rx - 2.0, (ry - 1.0).max(1.0)), c);
    }
}
