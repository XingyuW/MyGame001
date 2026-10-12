//! Authored layout shared by map generation and presentation. Decorations with collision are
//! written into TMX; the renderer never invents passability. Keep both bases equally supplied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Detail {
    Plain,
    Road,
    Grove,
    Village,
}
pub fn detail(w: u32, h: u32, x: u32, y: u32) -> Detail {
    let (w, h, x, y) = (w as i32, h as i32, x as i32, y as i32);
    if w == 64
        && h == 64
        && [(15, 37), (52, 42)]
            .iter()
            .any(|(sx, sy)| (x - sx).abs() <= 10 && (y - sy).abs() <= 10)
    {
        return Detail::Plain;
    }
    let (x, y) = if (y, x) <= (h - 1 - y, w - 1 - x) {
        (x, y)
    } else {
        (w - 1 - x, h - 1 - y)
    };
    let (sx, sy) = match (w, h) {
        (56, 56) => (9, 9),
        (64, 48) => (8, 21),
        _ => (15, 37),
    };
    // Generous base apron keeps initial forces, expansion and refinery exits clear.
    if ((x - sx).abs() <= 10 && (y - sy).abs() <= 10)
        || ((x - (w - 1 - sx)).abs() <= 10 && (y - (h - 1 - sy)).abs() <= 10)
    {
        return Detail::Plain;
    }
    let road = if h == 48 {
        (x >= 15 && [6, 23, 40].iter().any(|v| (y - v).abs() <= 1))
            || ((x - 15).abs() <= 1 && (6..=40).contains(&y))
    } else {
        ((x - w / 4).abs() <= 1 && y >= h / 4)
            || ((y - h / 4).abs() <= 1 && x >= w / 4)
            || ((x - 3 * w / 4).abs() <= 1 && y >= h / 4)
            || ((y - 3 * h / 4).abs() <= 1 && x >= w / 4)
    };
    if road {
        return Detail::Road;
    }
    // Small roadside settlement blocks, separated by accessible lanes and gardens.
    if h != 48
        && (w / 4 + 4..=w / 4 + 10).contains(&x)
        && (h / 4 - 6..=h / 4 - 3).contains(&y)
        && x % 3 != 0
        && y % 3 != 0
    {
        return Detail::Village;
    }
    let grove = (x - w + 10).pow(2) + (y - 8).pow(2) < 30
        || (x - 7).pow(2) + (y - h / 2).pow(2) < 20
        || (x - w / 2).pow(2) + (y - 5).pow(2) < 16;
    if grove { Detail::Grove } else { Detail::Plain }
}
