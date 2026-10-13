//! Cell looks: the meaning of the presentation-only `look` tile property (`sim::LoadedMap::looks`),
//! shared by the map generators and the renderer (`battlefield.rs`). Passability, water, building
//! ground and the amount of ore stay the tile's own properties; of a look the simulation reads
//! the destructible scenery (`sim::Props`) and, since RA5, whether the ore of a cell is gems
//! ([`Feature::Gems`], `sim::LOOK_FEATURE_GEMS`).
//!
//! A look packs the ground in its low four bits and the feature standing on it in its high four:
//! `look = ground | feature << 4`. Look 0 is "auto": the renderer derives both from the terrain,
//! which is how maps without the property (test fixtures) are drawn.

/// The ground a cell shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ground {
    /// Derived from the terrain.
    Auto = 0,
    Grass = 1,
    /// Bare earth: tracks, farmland, scrub.
    Dirt = 2,
    /// Beaches and desert.
    Sand = 3,
    Snow = 4,
    /// Concrete plazas and sidewalks of cities.
    Pavement = 5,
    Water = 6,
    /// A frozen lake or sea: units cross it, nothing is built on it.
    Ice = 7,
    /// A grain field: farmland in crop.
    Crops = 8,
    /// Waterlogged lowland: units wade through it, nothing is built on it.
    Marsh = 9,
    /// Volcanic ash and black sand.
    Ash = 10,
}

/// What stands on the ground. Features with collision are impassable tiles in the map file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Feature {
    None = 0,
    Ore = 1,
    /// Rocky terrace; drawn as rock, snowy rock or desert mesa after the ground.
    Cliff = 2,
    Road = 3,
    /// Trees; drawn as broadleaf and pine, snowy pine or palms after the ground.
    Forest = 4,
    /// A countryside cottage.
    House = 5,
    /// A low city block: shops, apartments, warehouses.
    City = 6,
    /// A high-rise city tower.
    Tower = 7,
    /// A bridge deck over water.
    Bridge = 8,
    /// A lava flow: impassable, and no sea for ships either.
    Lava = 9,
    /// Gems (RA5): ore of twice the value that neither grows nor spreads. The one feature that
    /// tells the simulation something the terrain does not: a cell with ore and this feature
    /// holds gems.
    Gems = 10,
}

pub const GROUNDS: [Ground; 11] = [
    Ground::Auto,
    Ground::Grass,
    Ground::Dirt,
    Ground::Sand,
    Ground::Snow,
    Ground::Pavement,
    Ground::Water,
    Ground::Ice,
    Ground::Crops,
    Ground::Marsh,
    Ground::Ash,
];

pub const FEATURES: [Feature; 11] = [
    Feature::None,
    Feature::Ore,
    Feature::Cliff,
    Feature::Road,
    Feature::Forest,
    Feature::House,
    Feature::City,
    Feature::Tower,
    Feature::Bridge,
    Feature::Lava,
    Feature::Gems,
];

pub const fn look(ground: Ground, feature: Feature) -> u8 {
    ground as u8 | (feature as u8) << 4
}

/// The ground and feature of a look, or `None` for a value no map may use.
pub fn decode(look: u8) -> Option<(Ground, Feature)> {
    let ground = *GROUNDS.get(usize::from(look & 15))?;
    let feature = *FEATURES.get(usize::from(look >> 4))?;
    Some((ground, feature))
}
