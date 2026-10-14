//! Material-preserving team paint. Authored faction paint is recoloured; concrete,
//! armour, glass, hazard markings and shadows retain their lighting. Inputs have passed I11.
use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use sim::PlayerId;

// The same ids as `crate::units::{HUMAN, OPPONENT}`; this module is also compiled on its own by
// the team paint test and contact-sheet example, which have no `units` module.
const HUMAN: PlayerId = PlayerId(0);
const OPPONENT: PlayerId = PlayerId(1);

/// One colour per player of the largest match (`sim::MAX_MATCH_PLAYERS`), all distinct.
///
/// The first eight are the colours of the eight-player matches, in their order. Lime and Ivory
/// were added for the ninth and tenth player as the two colours farthest from those eight and
/// from each other (CIE76 distance in L*a*b*: Lime 42 from its nearest, Gold; Ivory 50 from its
/// nearest, Cyan; the nearest of the eight, Red and Orange, are 30 apart), and apart from the
/// minimap's terrain (a pure white would vanish on snow). Paint is the colour plus the authored
/// shade of a pixel, from 42 below it to 53 above ([`paint_pixel`]): Lime's darkest channel (48)
/// keeps every shadow, and Ivory's brightest (240) clips only the brightest highlights, about
/// one painted pixel in twenty of the shipped sprites.
pub const TEAM_PALETTE: [(&str, [u8; 3]); 10] = [
    ("Blue", [42, 133, 202]),
    ("Red", [195, 66, 48]),
    ("Gold", [212, 175, 52]),
    ("Green", [62, 170, 92]),
    ("Purple", [162, 86, 190]),
    ("Cyan", [45, 185, 191]),
    ("Orange", [228, 118, 30]),
    ("Pink", [226, 112, 178]),
    ("Lime", [168, 220, 48]),
    ("Ivory", [240, 230, 196]),
];

/// The human player's palette choice. Every player has its own colour: the human the chosen one,
/// computer player k (`PlayerId(k)`) the k-th colour after it, so no two players of a match share
/// a colour and the first opponent keeps the colour it had in one-against-one matches.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamColors(pub usize);
impl TeamColors {
    /// The palette index of `player`.
    pub fn index_of(&self, player: PlayerId) -> usize {
        team_color_index(self.0, player)
    }
    /// The colour of `player`.
    pub fn rgb_of(&self, player: PlayerId) -> [u8; 3] {
        if player == sim::NEUTRAL {
            [156, 156, 148]
        } else {
            TEAM_PALETTE[self.index_of(player)].1
        }
    }
    /// The human player's colour, or the first opponent's.
    pub fn rgb(&self, human: bool) -> [u8; 3] {
        self.rgb_of(if human { HUMAN } else { OPPONENT })
    }
}

/// The palette index of `player` when the human player chose `choice`.
pub fn team_color_index(choice: usize, player: PlayerId) -> usize {
    (choice + usize::from(player.0)) % TEAM_PALETTE.len()
}

#[derive(Resource, Default)]
struct OriginalPaint(std::collections::HashMap<AssetId<Image>, Image>);

/// Repainted copies of a sprite for the players beyond the first two, made on first use: each
/// HD sheet is several megabytes, so a copy per colour of every sprite would cost gigabytes, while
/// a computer player only ever shows the few types it owns. Keyed by the sprite's human copy and
/// the palette index; cleared when a match ends.
#[derive(Resource, Default)]
pub struct TeamSprites(std::collections::HashMap<(AssetId<Image>, usize), Handle<Image>>);

impl TeamSprites {
    /// Copies painted so far.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

/// Picks the sprite of a unit or building for its owner. It only reads, so the view systems that
/// use it need no order among themselves; a missing copy is painted by a queued command at the
/// end of the schedule, and the views pick it up on the next frame.
#[derive(SystemParam)]
pub struct TeamPaint<'w, 's> {
    colors: Option<Res<'w, TeamColors>>,
    cache: Option<Res<'w, TeamSprites>>,
    commands: Commands<'w, 's>,
}

impl TeamPaint<'_, '_> {
    /// The copy of the sprite pair (`human`, `enemy`: the human player's and the first
    /// opponent's copy) for `owner`.
    pub fn sprite(
        &mut self,
        owner: PlayerId,
        human: &Handle<Image>,
        enemy: &Handle<Image>,
    ) -> Handle<Image> {
        if owner == HUMAN {
            return human.clone();
        }
        if owner == OPPONENT {
            return enemy.clone();
        }
        let colors = self.colors.as_deref().copied().unwrap_or_default();
        let index = if owner == sim::NEUTRAL {
            TEAM_PALETTE.len()
        } else {
            colors.index_of(owner)
        };
        if let Some(handle) = self
            .cache
            .as_deref()
            .and_then(|cache| cache.0.get(&(human.id(), index)))
        {
            return handle.clone();
        }
        let id = human.id();
        self.commands
            .queue(move |world: &mut World| paint_team_sprite(world, id, index));
        enemy.clone()
    }
}

/// Paints the copy of the sprite whose human copy is `id` in palette colour `index`, unless it
/// exists already (several views may ask for it in one frame).
fn paint_team_sprite(world: &mut World, id: AssetId<Image>, index: usize) {
    if world
        .get_resource::<TeamSprites>()
        .is_none_or(|cache| cache.0.contains_key(&(id, index)))
    {
        return;
    }
    let Some(source) = world
        .get_resource::<OriginalPaint>()
        .and_then(|originals| originals.0.get(&id))
        .map(|source| {
            painted(
                source,
                TEAM_PALETTE
                    .get(index)
                    .map_or([156, 156, 148], |entry| entry.1),
            )
        })
    else {
        return;
    };
    let handle = world.resource_mut::<Assets<Image>>().add(source);
    world
        .resource_mut::<TeamSprites>()
        .0
        .insert((id, index), handle);
}

/// A copy of `source` with its authored team paint in `rgb`.
fn painted(source: &Image, rgb: [u8; 3]) -> Image {
    let mut painted = source.clone();
    if let Some(data) = painted.data.as_mut() {
        for pixel in data.as_chunks_mut::<4>().0 {
            if pixel[0] > pixel[1].saturating_add(65)
                && pixel[0] > pixel[2].saturating_add(70)
                && pixel[0] < 220
            {
                let light = i16::from(pixel[0]) - 168;
                for c in 0..3 {
                    pixel[c] = (i16::from(rgb[c]) + light).clamp(0, 255) as u8;
                }
            } else {
                paint_pixel(pixel, rgb);
            }
        }
    }
    painted
}

/// Repaint from preserved originals on each start/restart, never from already recoloured pixels.
pub fn apply_team_colors(
    world: &mut World,
    choice: usize,
    pairs: Vec<(Handle<Image>, Handle<Image>)>,
) {
    let colors = TeamColors(choice);
    let mut originals = world.remove_resource::<OriginalPaint>().unwrap_or_default();
    let mut images = world.resource_mut::<Assets<Image>>();
    for (human, enemy) in pairs {
        if let Some(original) = images.get(&human).cloned() {
            let source = originals.0.entry(human.id()).or_insert(original);
            for (handle, rgb) in [(human, colors.rgb(true)), (enemy, colors.rgb(false))] {
                let painted = painted(source, rgb);
                if let Some(mut image) = images.get_mut(&handle) {
                    *image = painted;
                }
            }
        }
    }
    world.insert_resource(originals);
    world.insert_resource(colors);
    // Copies for further computer players are painted on first use for this match's colours.
    world.insert_resource(TeamSprites::default());
}

pub fn team_pixel(pixel: &mut [u8]) {
    paint_pixel(pixel, TEAM_PALETTE[1].1);
}
fn paint_pixel(pixel: &mut [u8], rgb: [u8; 3]) {
    if pixel[2] > pixel[0].saturating_add(60) && pixel[1] > pixel[0].saturating_add(35) {
        let light = i16::from(pixel[0]) - 42;
        for c in 0..3 {
            pixel[c] = (i16::from(rgb[c]) + light).clamp(0, 255) as u8;
        }
    }
}

/// Decode only bytes already validated by check_png_file. Keep CPU pixels for icons and review.
pub fn sprite_pair(
    bytes: &[u8],
    images: &mut Assets<Image>,
) -> Result<(Handle<Image>, Handle<Image>), String> {
    let image = Image::from_buffer(
        bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::linear(),
        RenderAssetUsages::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut enemy = image.clone();
    if let Some(pixels) = enemy.data.as_mut() {
        for pixel in pixels.as_chunks_mut::<4>().0 {
            team_pixel(pixel);
        }
    }
    Ok((images.add(image), images.add(enemy)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ra5_neutral_paint_is_gray_then_capture_uses_the_new_owner_without_palette_indexing() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let source = Image {
            data: Some(vec![42, 133, 202, 255]),
            ..default()
        };
        let human = world.resource_mut::<Assets<Image>>().add(source.clone());
        let enemy = world.resource_mut::<Assets<Image>>().add(source);
        apply_team_colors(&mut world, 0, vec![(human.clone(), enemy.clone())]);
        let mut state: bevy::ecs::system::SystemState<TeamPaint> =
            bevy::ecs::system::SystemState::new(&mut world);
        state
            .get_mut(&mut world)
            .expect("paint")
            .sprite(sim::NEUTRAL, &human, &enemy);
        state.apply(&mut world);
        let neutral =
            state
                .get_mut(&mut world)
                .expect("paint")
                .sprite(sim::NEUTRAL, &human, &enemy);
        assert_eq!(
            world
                .resource::<Assets<Image>>()
                .get(&neutral)
                .expect("gray")
                .data
                .as_ref()
                .expect("pixels")[..3],
            [156, 156, 148]
        );
        assert_eq!(
            state
                .get_mut(&mut world)
                .expect("paint")
                .sprite(HUMAN, &human, &enemy),
            human
        );
        assert_eq!(TeamColors(9).rgb_of(sim::NEUTRAL), [156, 156, 148]);
    }

    #[test]
    fn every_player_of_the_largest_match_has_its_own_colour_for_every_choice() {
        for choice in 0..TEAM_PALETTE.len() {
            let colors = TeamColors(choice);
            let indices: std::collections::BTreeSet<usize> = (0..sim::MAX_MATCH_PLAYERS)
                .map(|k| colors.index_of(PlayerId(k as u8)))
                .collect();
            assert_eq!(indices.len(), sim::MAX_MATCH_PLAYERS, "choice {choice}");
            assert_eq!(colors.index_of(HUMAN), choice);
            // The first opponent keeps the colour of the one-against-one format.
            assert_eq!(colors.index_of(OPPONENT), (choice + 1) % TEAM_PALETTE.len());
        }
    }

    /// Squared distance of two colours in sRGB bytes, weighted as the eye weighs the channels
    /// ("redmean"): cheap, and close enough to tell colours that can be confused.
    fn colour_distance(a: [u8; 3], b: [u8; 3]) -> i64 {
        let mean = (i64::from(a[0]) + i64::from(b[0])) / 2;
        let d = |c: usize| i64::from(a[c]) - i64::from(b[c]);
        ((512 + mean) * d(0) * d(0)) / 256 + 4 * d(1) * d(1) + ((767 - mean) * d(2) * d(2)) / 256
    }

    #[test]
    fn the_palette_has_a_distinct_colour_for_each_of_the_ten_players() {
        assert_eq!(TEAM_PALETTE.len(), sim::MAX_MATCH_PLAYERS);
        // The colours of the eight-player matches keep their places.
        let names: Vec<&str> = TEAM_PALETTE.iter().map(|colour| colour.0).collect();
        assert_eq!(
            names,
            [
                "Blue", "Red", "Gold", "Green", "Purple", "Cyan", "Orange", "Pink", "Lime", "Ivory"
            ]
        );
        // The two new colours are farther from every other colour than the nearest two of the
        // first eight are from each other.
        let first = &TEAM_PALETTE[..8];
        let nearest = first
            .iter()
            .enumerate()
            .flat_map(|(index, a)| first[..index].iter().map(|b| colour_distance(a.1, b.1)))
            .min()
            .expect("pairs");
        assert!(nearest > 0);
        for (index, (name, rgb)) in TEAM_PALETTE.iter().enumerate().skip(8) {
            for (other, other_rgb) in &TEAM_PALETTE[..index] {
                assert!(
                    colour_distance(*rgb, *other_rgb) > nearest,
                    "{name} and {other}: {} <= {nearest}",
                    colour_distance(*rgb, *other_rgb)
                );
            }
        }
        // Painted on the authored ramp (the palette colour at shade 0, 42 below to 53 above),
        // every colour keeps a light and a dark side: the mid shades of no colour clip, also not
        // of the lightest (Ivory) and of the one with the darkest channel (Blue's red).
        for (name, rgb) in TEAM_PALETTE {
            let shade = |red: u8| {
                let mut pixel = [red, red + 91, red + 160, 255];
                paint_pixel(&mut pixel, rgb);
                [pixel[0], pixel[1], pixel[2]]
            };
            assert_eq!(shade(42), rgb, "{name}: the colour itself");
            let (dark, light) = (shade(27), shade(57));
            for channel in 0..3 {
                assert!(
                    dark[channel] < rgb[channel] && rgb[channel] < light[channel],
                    "{name}: {dark:?} {rgb:?} {light:?}"
                );
            }
        }
    }

    #[test]
    fn further_computer_players_get_their_own_copies_painted_on_first_use() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let image = Image {
            data: Some(vec![42, 133, 202, 255, 126, 139, 142, 255]),
            ..default()
        };
        let human = world.resource_mut::<Assets<Image>>().add(image.clone());
        let enemy = world.resource_mut::<Assets<Image>>().add(image);
        apply_team_colors(&mut world, 0, vec![(human.clone(), enemy.clone())]);
        let mut state: bevy::ecs::system::SystemState<TeamPaint> =
            bevy::ecs::system::SystemState::new(&mut world);
        // The first two players use the pair; a third player's copy is queued, then cached.
        {
            let mut paint = state.get_mut(&mut world).expect("valid");
            assert_eq!(paint.sprite(HUMAN, &human, &enemy), human);
            assert_eq!(paint.sprite(OPPONENT, &human, &enemy), enemy);
            assert_eq!(paint.sprite(PlayerId(2), &human, &enemy), enemy);
            assert_eq!(paint.sprite(PlayerId(2), &human, &enemy), enemy);
        }
        state.apply(&mut world);
        assert_eq!(world.resource::<TeamSprites>().len(), 1, "painted once");
        let third = state
            .get_mut(&mut world)
            .expect("valid")
            .sprite(PlayerId(2), &human, &enemy);
        assert!(third != human && third != enemy);
        let pixels = |handle: &Handle<Image>| {
            world
                .resource::<Assets<Image>>()
                .get(handle)
                .unwrap()
                .data
                .clone()
                .unwrap()
        };
        let colors = *world.resource::<TeamColors>();
        assert_eq!(pixels(&third)[..3], colors.rgb_of(PlayerId(2)));
        // Neutral material is untouched; the three players' paints all differ.
        assert_eq!(pixels(&third)[4..8], [126, 139, 142, 255]);
        assert_ne!(pixels(&third)[..3], pixels(&human)[..3]);
        assert_ne!(pixels(&third)[..3], pixels(&enemy)[..3]);
        // A new match starts with an empty cache (the copies are freed).
        apply_team_colors(&mut world, 3, vec![(human.clone(), enemy.clone())]);
        assert_eq!(world.resource::<TeamSprites>().len(), 0);
    }

    #[test]
    fn palette_restarts_repaint_from_source_and_preserve_neutral_materials() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let image = Image {
            data: Some(vec![
                42, 133, 202, 255, 126, 139, 142, 255, 168, 43, 32, 255,
            ]),
            ..default()
        };
        let human = world.resource_mut::<Assets<Image>>().add(image.clone());
        let enemy = world.resource_mut::<Assets<Image>>().add(image);
        for choice in [1, 4, 0, 5, 2, 3, 1] {
            apply_team_colors(&mut world, choice, vec![(human.clone(), enemy.clone())]);
            let colors = world.resource::<TeamColors>();
            assert_ne!(colors.rgb(true), colors.rgb(false));
            for (handle, owner) in [(&human, true), (&enemy, false)] {
                let data = world
                    .resource::<Assets<Image>>()
                    .get(handle)
                    .unwrap()
                    .data
                    .as_ref()
                    .unwrap();
                assert_eq!(data[..3], colors.rgb(owner));
                assert_eq!(data[4..8], [126, 139, 142, 255]);
                assert_eq!(data[8..11], colors.rgb(owner));
                assert_eq!(data[3], 255);
            }
        }
    }
    #[test]
    fn team_paint_preserves_materials_alpha_and_shading() {
        for mut pixel in [
            [126, 139, 142, 255],
            [166, 219, 201, 255],
            [140, 140, 140, 255],
            [20, 28, 22, 90],
            [66, 111, 125, 255],
        ] {
            let before = pixel;
            team_pixel(&mut pixel);
            assert_eq!(pixel, before);
        }
        let mut blue = [42, 133, 202, 255];
        team_pixel(&mut blue);
        assert_eq!(blue, [195, 66, 48, 255]);
        let mut shadow = [24, 115, 184, 190];
        team_pixel(&mut shadow);
        assert_eq!(shadow, [177, 48, 30, 190]);
    }
}
