//! Temperate grass, mineral clusters, rippled water and faceted stone; no map rules live here.
fn hash(x: u32, y: u32) -> u32 {
    let mut n = x.wrapping_mul(0x85ebca6b) ^ y.wrapping_mul(0xc2b2ae35);
    n ^= n >> 16;
    n = n.wrapping_mul(0x7feb352d);
    n ^ (n >> 15)
}
fn shade(c: [u8; 3], d: i32) -> [u8; 3] {
    c.map(|v| (i32::from(v) + d).clamp(0, 255) as u8)
}
pub fn pixel(kind: u32, x: u32, y: u32) -> [u8; 3] {
    let n = hash(x, y);
    let grain = (n % 9) as i32 - 4;
    match kind {
        0 => {
            let patch = (hash(x / 7, y / 4) % 13) as i32 - 6;
            let c = if n.is_multiple_of(23) {
                [105, 115, 64]
            } else {
                [82, 105, 59]
            };
            shade(c, grain + patch)
        }
        1 => {
            // Several angular mineral nuggets on earthy ground, with separate lit/shaded faces.
            let mut c = shade([102, 92, 59], grain);
            for (cx, cy, r) in [
                (18_i32, 15_i32, 6),
                (32, 12, 7),
                (43, 17, 5),
                (29, 23, 4),
                (46, 10, 3),
            ] {
                let dx = x as i32 - cx;
                let dy = (y as i32 - cy) * 2;
                if dx.abs() + dy.abs() < r {
                    c = if dx + dy < 0 {
                        [227, 191, 84]
                    } else {
                        [155, 119, 44]
                    };
                }
            }
            c
        }
        2 => {
            let ripple = (y * 3 + x / 9) % 13;
            shade(
                if ripple < 2 {
                    [67, 112, 129]
                } else {
                    [44, 82, 99]
                },
                grain / 2,
            )
        }
        3 => {
            let mut c = shade([95, 98, 81], grain);
            for (cx, cy, r) in [
                (19_i32, 15_i32, 11),
                (34, 11, 12),
                (43, 20, 10),
                (29, 24, 7),
            ] {
                let dx = x as i32 - cx;
                let dy = (y as i32 - cy) * 2;
                if dx.abs() + dy.abs() < r {
                    c = shade(
                        if dx + dy < 0 {
                            [151, 151, 128]
                        } else {
                            [80, 88, 77]
                        },
                        grain,
                    );
                }
            }
            c
        }
        _ => shade([162, 150, 106], grain + (hash(x / 4, y / 3) % 5) as i32),
    }
}
