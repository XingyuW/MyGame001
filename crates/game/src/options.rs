//! Options (PLAN.md section 7, M7: "options"; docs/DECISIONS.md D52): the volume settings of M5
//! (master, music, and effects levels, mute), edge scrolling on or off, and the scroll speed, set
//! on an options screen reached from the main menu (the volume also from the sidebar), and stored
//! between sessions in a settings file.
//!
//! The settings file is the game's only file that it writes, and the only file it reads that is not
//! part of the assets. It is read with this module's own parser, not a third-party one, so invariant
//! I11 has no hand-off to restrict; the parser accepts exactly this format and nothing else
//! ([`parse_settings`]):
//!
//! ```text
//! # MyGame001 settings
//! version = 1
//! master_volume = 8
//! music_volume = 5
//! effects_volume = 7
//! muted = false
//! edge_scrolling = true
//! scroll_speed = 5
//! ```
//!
//! * at most [`MAX_SETTINGS_BYTES`] bytes, read with that bound, from a regular file (a directory,
//!   device, or named pipe is rejected before it is opened);
//! * printable ASCII, space, tab, line feed, and carriage return only;
//! * every line is empty, a comment starting with `#`, or `key = value` (spaces around `=`
//!   optional); every key of [`KEYS`] exactly once, no other key (no silent defaults);
//! * `version` is [`SETTINGS_VERSION`]; volume levels are integers from 0 to
//!   [`crate::audio::VOLUME_STEPS`]; the scroll speed from 1 to [`SCROLL_STEPS`]; `muted` and
//!   `edge_scrolling` are `true` or `false`; integers are decimal digits without sign or leading
//!   zeros.
//!
//! Every rejection is a typed [`SettingsError`] naming the file, the line, the key, and the reason.
//! A missing file means the defaults (logged). An invalid file is reported with a warning naming the
//! file and the reason, the defaults are used, and the file is replaced (with a log line) only when
//! the player changes an option; nothing is lost silently. The file is written whole to a temporary
//! file beside it and renamed over it, so a crash cannot leave half a file.
//!
//! The file lives in `~/Library/Application Support/MyGame001/settings.txt` (version 1.0 targets
//! macOS only, PLAN.md section 12 item 3). Without `HOME` the options still work for the session
//! and a warning says that they are not stored. A smoke run uses a file of its own in the temporary
//! directory and removes it, so that it never reads or changes the player's settings.

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::audio::{Channel, VOLUME_STEPS, VolumeSettings};
use crate::skirmish::Screen;

/// The version of the settings format this game reads and writes.
pub const SETTINGS_VERSION: u32 = 1;

/// Largest settings file read.
pub const MAX_SETTINGS_BYTES: u64 = 4096;

/// Scroll speeds run from 1 to this; [`DEFAULT_SCROLL_SPEED`] is the speed of M1 to M6.
pub const SCROLL_STEPS: u8 = 10;

/// The scroll speed of M1 to M6 (`crate::camera::PAN_SPEED`).
pub const DEFAULT_SCROLL_SPEED: u8 = 5;

/// The keys of the settings file, in the order they are written.
pub const KEYS: [&str; 7] = [
    "version",
    "master_volume",
    "music_volume",
    "effects_volume",
    "muted",
    "edge_scrolling",
    "scroll_speed",
];

/// How the camera follows the mouse and keys.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraOptions {
    /// Whether the cursor at a window edge scrolls the map (D10).
    pub edge_scrolling: bool,
    /// From 1 to [`SCROLL_STEPS`].
    pub scroll_speed: u8,
}

impl Default for CameraOptions {
    fn default() -> Self {
        Self {
            edge_scrolling: false,
            scroll_speed: DEFAULT_SCROLL_SPEED,
        }
    }
}

impl CameraOptions {
    /// The factor applied to the pan speed: 1 at the default speed, from 0.2 to 2.
    pub fn speed_factor(&self) -> f32 {
        f32::from(self.scroll_speed.clamp(1, SCROLL_STEPS)) / f32::from(DEFAULT_SCROLL_SPEED)
    }
}

/// Everything the settings file stores.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub volume: VolumeSettings,
    pub camera: CameraOptions,
}

/// Why a settings file was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsErrorKind {
    /// The file could not be read, or is not a regular file.
    Read(String),
    FileTooLarge {
        max_bytes: u64,
    },
    /// A byte other than printable ASCII, space, tab, line feed, or carriage return.
    NotText {
        byte: u8,
    },
    /// A line that is not empty, a comment, or `key = value`.
    Syntax,
    UnknownKey,
    DuplicateKey,
    MissingKey,
    InvalidValue {
        found: String,
        expected: String,
    },
}

impl fmt::Display for SettingsErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(f, "cannot read the file: {error}"),
            Self::FileTooLarge { max_bytes } => {
                write!(f, "the file is larger than {max_bytes} bytes")
            }
            Self::NotText { byte } => write!(
                f,
                "byte 0x{byte:02x} is not supported, expected printable ASCII text"
            ),
            Self::Syntax => write!(f, "expected an empty line, a comment, or key = value"),
            Self::UnknownKey => write!(f, "unknown key"),
            Self::DuplicateKey => write!(f, "the key appears more than once"),
            Self::MissingKey => write!(f, "required key is missing"),
            Self::InvalidValue { found, expected } => {
                write!(f, "value {found:?} is not supported, expected {expected}")
            }
        }
    }
}

/// A rejected settings file: the file, the line (1-based; 0 for the whole file), the key, and the
/// reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsError {
    pub file: PathBuf,
    pub line: usize,
    pub key: String,
    pub kind: SettingsErrorKind,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "settings file {}", self.file.display())?;
        if self.line > 0 {
            write!(f, ": line {}", self.line)?;
        }
        if !self.key.is_empty() {
            write!(f, ": {}", self.key)?;
        }
        write!(f, ": {}", self.kind)
    }
}

impl std::error::Error for SettingsError {}

/// A problem found by the parser: the line, the key, and the reason.
pub type SettingsProblem = (usize, String, SettingsErrorKind);

fn number(value: &str, min: u8, max: u8) -> Option<u8> {
    let digits = !value.is_empty()
        && value.len() <= 3
        && value.bytes().all(|b| b.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'));
    digits
        .then(|| value.parse::<u8>().ok())
        .flatten()
        .filter(|level| (min..=max).contains(level))
}

fn boolean(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Parses the text of a settings file (the format of the module documentation).
pub fn parse_settings(bytes: &[u8]) -> Result<Settings, SettingsProblem> {
    if let Some(&byte) = bytes
        .iter()
        .find(|&&b| !(b.is_ascii_graphic() || matches!(b, b' ' | b'\t' | b'\n' | b'\r')))
    {
        let line = 1 + bytes
            .iter()
            .take_while(|&&b| b != byte)
            .filter(|&&b| b == b'\n')
            .count();
        return Err((line, String::new(), SettingsErrorKind::NotText { byte }));
    }
    // Printable ASCII only, so the bytes are UTF-8.
    let text = String::from_utf8_lossy(bytes);
    let mut values: [Option<(usize, &str)>; KEYS.len()] = [None; KEYS.len()];
    for (index, raw) in text.split('\n').enumerate() {
        let line = index + 1;
        let content = raw.strip_suffix('\r').unwrap_or(raw).trim();
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        let Some((key, value)) = content.split_once('=') else {
            return Err((line, String::new(), SettingsErrorKind::Syntax));
        };
        let (key, value) = (key.trim(), value.trim());
        if key.is_empty() || value.is_empty() || value.contains('=') {
            return Err((line, key.to_owned(), SettingsErrorKind::Syntax));
        }
        let Some(slot) = KEYS.iter().position(|&known| known == key) else {
            return Err((line, key.to_owned(), SettingsErrorKind::UnknownKey));
        };
        if values[slot].is_some() {
            return Err((line, key.to_owned(), SettingsErrorKind::DuplicateKey));
        }
        values[slot] = Some((line, value));
    }
    let get = |slot: usize| {
        values[slot].ok_or_else(|| (0, KEYS[slot].to_owned(), SettingsErrorKind::MissingKey))
    };
    let invalid = |slot: usize, line: usize, value: &str, expected: String| {
        (
            line,
            KEYS[slot].to_owned(),
            SettingsErrorKind::InvalidValue {
                found: value.to_owned(),
                expected,
            },
        )
    };
    // The version first, so that a file of another version is reported as such.
    let (line, value) = get(0)?;
    if value != SETTINGS_VERSION.to_string() {
        return Err(invalid(0, line, value, SETTINGS_VERSION.to_string()));
    }
    let level = |slot: usize| -> Result<u8, SettingsProblem> {
        let (line, value) = get(slot)?;
        number(value, 0, VOLUME_STEPS)
            .ok_or_else(|| invalid(slot, line, value, format!("0 to {VOLUME_STEPS}")))
    };
    let flag = |slot: usize| -> Result<bool, SettingsProblem> {
        let (line, value) = get(slot)?;
        boolean(value).ok_or_else(|| invalid(slot, line, value, "true or false".into()))
    };
    let volume = VolumeSettings {
        master: level(1)?,
        music: level(2)?,
        effects: level(3)?,
        muted: flag(4)?,
    };
    let edge_scrolling = flag(5)?;
    let (line, value) = get(6)?;
    let scroll_speed = number(value, 1, SCROLL_STEPS)
        .ok_or_else(|| invalid(6, line, value, format!("1 to {SCROLL_STEPS}")))?;
    Ok(Settings {
        volume,
        camera: CameraOptions {
            edge_scrolling,
            scroll_speed,
        },
    })
}

/// The text of the settings file for `settings`; [`parse_settings`] reads it back unchanged.
pub fn format_settings(settings: &Settings) -> String {
    let Settings { volume, camera } = settings;
    format!(
        "# MyGame001 settings, written by the game (docs/DECISIONS.md D52).\n\
         version = {SETTINGS_VERSION}\n\
         master_volume = {}\n\
         music_volume = {}\n\
         effects_volume = {}\n\
         muted = {}\n\
         edge_scrolling = {}\n\
         scroll_speed = {}\n",
        volume.master.min(VOLUME_STEPS),
        volume.music.min(VOLUME_STEPS),
        volume.effects.min(VOLUME_STEPS),
        volume.muted,
        camera.edge_scrolling,
        camera.scroll_speed.clamp(1, SCROLL_STEPS),
    )
}

/// Reads the settings file at `path`, or `Ok(None)` if there is none.
pub fn read_settings(path: &Path) -> Result<Option<Settings>, SettingsError> {
    let error = |line: usize, key: String, kind: SettingsErrorKind| SettingsError {
        file: path.to_owned(),
        line,
        key,
        kind,
    };
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(_) => std::fs::metadata(path)
            .map_err(|e| error(0, String::new(), SettingsErrorKind::Read(e.to_string())))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(error(
                0,
                String::new(),
                SettingsErrorKind::Read(e.to_string()),
            ));
        }
    };
    if !metadata.is_file() {
        return Err(error(
            0,
            String::new(),
            SettingsErrorKind::Read("not a regular file".into()),
        ));
    }
    let file = std::fs::File::open(path)
        .map_err(|e| error(0, String::new(), SettingsErrorKind::Read(e.to_string())))?;
    let mut bytes = Vec::new();
    file.take(MAX_SETTINGS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| error(0, String::new(), SettingsErrorKind::Read(e.to_string())))?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_SETTINGS_BYTES) {
        return Err(error(
            0,
            String::new(),
            SettingsErrorKind::FileTooLarge {
                max_bytes: MAX_SETTINGS_BYTES,
            },
        ));
    }
    parse_settings(&bytes)
        .map(Some)
        .map_err(|(line, key, kind)| error(line, key, kind))
}

/// Writes `settings` to `path`: the directory is created if needed, and the text is written to a
/// temporary file beside it that is then renamed over it.
pub fn write_settings(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary = path.with_extension("txt.tmp");
    std::fs::write(&temporary, format_settings(settings))?;
    std::fs::rename(&temporary, path)
}

/// `~/Library/Application Support/MyGame001/settings.txt`, or `None` without `HOME`.
pub fn default_settings_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|home| !home.is_empty())?;
    Some(
        PathBuf::from(home)
            .join("Library/Application Support/MyGame001")
            .join("settings.txt"),
    )
}

/// The settings file of this run and what has been stored in it.
#[derive(Resource, Clone, Debug, Default)]
pub struct SettingsFile {
    /// `None`: the settings are not stored (no `HOME`).
    pub path: Option<PathBuf>,
    /// The settings as last read or written; a change of the options is written when they differ.
    pub stored: Settings,
    /// Writes so far, and the last write's error.
    pub writes: u64,
    pub last_error: Option<String>,
    /// The file on disk was rejected when it was read; the next write replaces it.
    pub replaces_invalid: bool,
}

pub struct OptionsPlugin {
    /// Where the settings are stored (`None`: not stored).
    pub path: Option<PathBuf>,
}

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraOptions>()
            .init_resource::<OptionsOpen>()
            .insert_resource(SettingsFile {
                path: self.path.clone(),
                ..default()
            })
            .add_systems(
                Startup,
                (
                    load_settings
                        .after(crate::skirmish::load_catalogue)
                        .before(crate::audio::setup_audio),
                    spawn_options.after(crate::skirmish::spawn_screens),
                ),
            )
            .add_systems(
                Update,
                (options_input, update_options)
                    .chain()
                    // The menu opens the options screen; the camera and the audio read the
                    // options the screen writes.
                    .before(crate::skirmish::screen_input)
                    .before(crate::camera::pan_camera)
                    .before(crate::audio::play_effects),
            )
            .add_systems(Last, save_settings);
    }
}

/// Startup: reads the settings file into the volume and camera options. A missing file leaves the
/// defaults; an invalid one is reported and leaves the defaults too.
fn load_settings(
    mut file: ResMut<SettingsFile>,
    mut volume: ResMut<VolumeSettings>,
    mut camera: ResMut<CameraOptions>,
) {
    let Some(path) = file.path.clone() else {
        warn!("HOME is not set: options are kept for this session only and are not stored");
        file.stored = Settings {
            volume: *volume,
            camera: *camera,
        };
        return;
    };
    let loaded = match read_settings(&path) {
        Ok(Some(settings)) => {
            info!("settings read from {}", path.display());
            settings
        }
        Ok(None) => {
            info!(
                "no settings file at {}; the defaults are used",
                path.display()
            );
            Settings::default()
        }
        Err(error) => {
            warn!(
                "{error}; the defaults are used, and the file is replaced when an option changes"
            );
            file.replaces_invalid = true;
            Settings::default()
        }
    };
    *volume = loaded.volume;
    *camera = loaded.camera;
    file.stored = loaded;
}

/// Writes the options to the settings file whenever they differ from what it holds.
fn save_settings(
    mut file: ResMut<SettingsFile>,
    volume: Res<VolumeSettings>,
    camera: Res<CameraOptions>,
) {
    let current = Settings {
        volume: *volume,
        camera: *camera,
    };
    if current == file.stored {
        return;
    }
    file.stored = current;
    let Some(path) = file.path.clone() else {
        return;
    };
    match write_settings(&path, &current) {
        Ok(()) => {
            file.writes = file.writes.saturating_add(1);
            file.last_error = None;
            if std::mem::take(&mut file.replaces_invalid) {
                info!(
                    "settings written to {}, replacing the invalid file",
                    path.display()
                );
            } else {
                info!("settings written to {}", path.display());
            }
        }
        Err(error) => {
            let message = format!("settings file {}: cannot write: {error}", path.display());
            error!("{message}");
            file.last_error = Some(message);
        }
    }
}

// --- The options screen ---------------------------------------------------------------------

/// Whether the options screen is shown (over the menu).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OptionsOpen(pub bool);

/// A row of the options screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptionRow {
    Master,
    Music,
    Effects,
    Sound,
    EdgeScrolling,
    ScrollSpeed,
}

impl OptionRow {
    pub const ALL: [OptionRow; 6] = [
        OptionRow::Master,
        OptionRow::Music,
        OptionRow::Effects,
        OptionRow::Sound,
        OptionRow::EdgeScrolling,
        OptionRow::ScrollSpeed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OptionRow::Master => "Master volume",
            OptionRow::Music => "Music volume",
            OptionRow::Effects => "Effects volume",
            OptionRow::Sound => "Sound",
            OptionRow::EdgeScrolling => "Edge scrolling",
            OptionRow::ScrollSpeed => "Scroll speed",
        }
    }

    /// Whether the row is switched on and off (one button) rather than stepped (two buttons).
    pub fn is_switch(self) -> bool {
        matches!(self, OptionRow::Sound | OptionRow::EdgeScrolling)
    }
}

/// A button of the options screen.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsButton {
    /// One step down (`false`) or up (`true`).
    Step(OptionRow, bool),
    /// Switch the row on or off.
    Switch(OptionRow),
    Back,
}

/// Buttons of the options screen: two per stepped row, one per switch, and Back.
pub const OPTIONS_BUTTONS: usize = 4 * 2 + 2 + 1;

/// What an options button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionsEffect {
    Set(Settings),
    Close,
    Nothing,
}

/// The effect of `button` on `settings` (pure). Steps stay within their ranges.
pub fn options_effect(button: OptionsButton, settings: Settings) -> OptionsEffect {
    let mut next = settings;
    match button {
        OptionsButton::Back => return OptionsEffect::Close,
        OptionsButton::Step(row, up) => match row {
            OptionRow::Master => next.volume.step(Channel::Master, up),
            OptionRow::Music => next.volume.step(Channel::Music, up),
            OptionRow::Effects => next.volume.step(Channel::Effects, up),
            OptionRow::ScrollSpeed => {
                let speed = next.camera.scroll_speed.clamp(1, SCROLL_STEPS);
                next.camera.scroll_speed = if up {
                    speed.saturating_add(1).min(SCROLL_STEPS)
                } else {
                    speed.saturating_sub(1).max(1)
                };
            }
            OptionRow::Sound | OptionRow::EdgeScrolling => return OptionsEffect::Nothing,
        },
        OptionsButton::Switch(row) => match row {
            OptionRow::Sound => next.volume.muted = !next.volume.muted,
            OptionRow::EdgeScrolling => {
                next.camera.edge_scrolling = !next.camera.edge_scrolling;
            }
            _ => return OptionsEffect::Nothing,
        },
    }
    OptionsEffect::Set(next)
}

/// The value shown in a row.
pub fn option_text(row: OptionRow, settings: &Settings) -> String {
    let on_off = |on: bool| if on { "On" } else { "Off" }.to_string();
    match row {
        OptionRow::Master => format!("{} / {VOLUME_STEPS}", settings.volume.master),
        OptionRow::Music => format!("{} / {VOLUME_STEPS}", settings.volume.music),
        OptionRow::Effects => format!("{} / {VOLUME_STEPS}", settings.volume.effects),
        OptionRow::Sound => on_off(!settings.volume.muted),
        OptionRow::EdgeScrolling => on_off(settings.camera.edge_scrolling),
        OptionRow::ScrollSpeed => format!("{} / {SCROLL_STEPS}", settings.camera.scroll_speed),
    }
}

/// The options screen's panel.
#[derive(Component, Debug)]
pub struct OptionsRoot;

/// The value text of a row.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionValue(pub OptionRow);

fn text(value: impl Into<String>, size: f32) -> (Text, TextFont, TextColor) {
    (
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(Color::srgb(0.92, 0.94, 0.97)),
    )
}

fn button(width: f32) -> impl Bundle {
    (
        Button,
        Node {
            width: Val::Px(width),
            height: Val::Px(32.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.24, 0.26, 0.3)),
    )
}

/// Spawns the options screen, hidden, above the menu.
pub(crate) fn spawn_options(mut commands: Commands) {
    commands
        .spawn((
            OptionsRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.06, 0.08)),
            GlobalZIndex(11),
            FocusPolicy::Block,
        ))
        .with_children(|panel| {
            panel.spawn(text("Options", 36.0));
            for row in OptionRow::ALL {
                panel
                    .spawn(Node {
                        column_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            Node {
                                width: Val::Px(160.0),
                                ..default()
                            },
                            children![text(row.label(), 18.0)],
                        ));
                        if row.is_switch() {
                            line.spawn((OptionsButton::Switch(row), button(140.0)))
                                .with_child((OptionValue(row), text("", 18.0)));
                        } else {
                            line.spawn((OptionsButton::Step(row, false), button(36.0)))
                                .with_child(text("-", 18.0));
                            line.spawn((
                                Node {
                                    width: Val::Px(60.0),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                children![(OptionValue(row), text("", 18.0))],
                            ));
                            line.spawn((OptionsButton::Step(row, true), button(36.0)))
                                .with_child(text("+", 18.0));
                        }
                    });
            }
            panel
                .spawn((OptionsButton::Back, button(200.0)))
                .with_child(text("Back", 20.0));
        });
}

/// Clicks on the options screen's buttons.
fn options_input(
    screen: Res<Screen>,
    mut open: ResMut<OptionsOpen>,
    mut volume: ResMut<VolumeSettings>,
    mut camera: ResMut<CameraOptions>,
    buttons: Query<(&Interaction, &OptionsButton), Changed<Interaction>>,
) {
    if *screen != Screen::Menu || !open.0 {
        return;
    }
    for (_, &button) in buttons
        .iter()
        .filter(|(interaction, _)| **interaction == Interaction::Pressed)
    {
        let settings = Settings {
            volume: *volume,
            camera: *camera,
        };
        match options_effect(button, settings) {
            OptionsEffect::Set(next) => {
                if next.volume != *volume {
                    *volume = next.volume;
                }
                if next.camera != *camera {
                    *camera = next.camera;
                }
            }
            OptionsEffect::Close => open.0 = false,
            OptionsEffect::Nothing => {}
        }
    }
}

/// Shows the options screen while it is open over the menu, with its values.
fn update_options(
    screen: Res<Screen>,
    open: Res<OptionsOpen>,
    volume: Res<VolumeSettings>,
    camera: Res<CameraOptions>,
    mut roots: Query<&mut Node, With<OptionsRoot>>,
    mut values: Query<(&mut Text, &OptionValue)>,
) {
    let shown = *screen == Screen::Menu && open.0;
    let display = if shown { Display::Flex } else { Display::None };
    for mut node in &mut roots {
        if node.display != display {
            node.display = display;
        }
    }
    let settings = Settings {
        volume: *volume,
        camera: *camera,
    };
    for (mut text, value) in &mut values {
        let wanted = option_text(value.0, &settings);
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Settings {
        Settings::default()
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mygame001_options_tests_{}_{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temporary directory");
        dir
    }

    #[test]
    fn the_written_file_reads_back_unchanged_for_every_value_at_both_ends() {
        let mut cases = vec![defaults()];
        for master in [0, VOLUME_STEPS] {
            for speed in [1, SCROLL_STEPS] {
                for flag in [false, true] {
                    cases.push(Settings {
                        volume: VolumeSettings {
                            master,
                            music: VOLUME_STEPS - master,
                            effects: master,
                            muted: flag,
                        },
                        camera: CameraOptions {
                            edge_scrolling: !flag,
                            scroll_speed: speed,
                        },
                    });
                }
            }
        }
        for settings in cases {
            let text = format_settings(&settings);
            assert_eq!(parse_settings(text.as_bytes()), Ok(settings), "{text}");
        }
        // Comments, blank lines, CRLF line ends, and spaces around `=` are accepted.
        let text = "# hello\r\n\r\nversion=1\r\nmaster_volume =8\nmusic_volume= 5\n  effects_volume = 7  \nmuted = false\nedge_scrolling = false\nscroll_speed = 5";
        assert_eq!(parse_settings(text.as_bytes()), Ok(defaults()));
    }

    #[test]
    fn malformed_settings_are_rejected_with_the_line_key_and_reason() {
        let good = format_settings(&defaults());
        let with = |from: &str, to: &str| {
            assert_eq!(good.matches(from).count(), 1, "{from}");
            good.replacen(from, to, 1)
        };
        let invalid = |found: &str, expected: &str| SettingsErrorKind::InvalidValue {
            found: found.into(),
            expected: expected.into(),
        };
        let cases: Vec<(String, usize, &str, SettingsErrorKind)> = vec![
            (
                with("version = 1", "version = 2"),
                2,
                "version",
                invalid("2", "1"),
            ),
            (
                with("version = 1", "version = 0"),
                2,
                "version",
                invalid("0", "1"),
            ),
            (
                with("master_volume = 8", "master_volume = 11"),
                3,
                "master_volume",
                invalid("11", "0 to 10"),
            ),
            (
                with("master_volume = 8", "master_volume = -1"),
                3,
                "master_volume",
                invalid("-1", "0 to 10"),
            ),
            (
                with("master_volume = 8", "master_volume = 08"),
                3,
                "master_volume",
                invalid("08", "0 to 10"),
            ),
            (
                with("master_volume = 8", "master_volume = +8"),
                3,
                "master_volume",
                invalid("+8", "0 to 10"),
            ),
            (
                with("music_volume = 5", "music_volume = 99999999999999999999"),
                4,
                "music_volume",
                invalid("99999999999999999999", "0 to 10"),
            ),
            (
                with("muted = false", "muted = no"),
                6,
                "muted",
                invalid("no", "true or false"),
            ),
            (
                with("scroll_speed = 5", "scroll_speed = 0"),
                8,
                "scroll_speed",
                invalid("0", "1 to 10"),
            ),
            (
                with("scroll_speed = 5", "scroll_speed = 11"),
                8,
                "scroll_speed",
                invalid("11", "1 to 10"),
            ),
            (
                with("muted = false\n", ""),
                0,
                "muted",
                SettingsErrorKind::MissingKey,
            ),
            (
                with("version = 1\n", ""),
                0,
                "version",
                SettingsErrorKind::MissingKey,
            ),
            (
                with("muted = false", "muted = false\nmuted = true"),
                7,
                "muted",
                SettingsErrorKind::DuplicateKey,
            ),
            (
                with("muted = false", "muted = false\nfullscreen = true"),
                7,
                "fullscreen",
                SettingsErrorKind::UnknownKey,
            ),
            (
                with("muted = false", "muted false"),
                6,
                "",
                SettingsErrorKind::Syntax,
            ),
            (
                with("muted = false", "muted = = false"),
                6,
                "muted",
                SettingsErrorKind::Syntax,
            ),
            (
                with("muted = false", " = false"),
                6,
                "",
                SettingsErrorKind::Syntax,
            ),
            (
                with("muted = false", "muted = \u{e9}"),
                6,
                "",
                SettingsErrorKind::NotText { byte: 0xc3 },
            ),
            (
                with("muted = false", "muted = fal\u{0}se"),
                6,
                "",
                SettingsErrorKind::NotText { byte: 0 },
            ),
        ];
        for (text, line, key, kind) in cases {
            assert_eq!(
                parse_settings(text.as_bytes()),
                Err((line, key.to_string(), kind)),
                "{text:?}"
            );
        }
        // The empty file lacks every key, the version first.
        assert_eq!(
            parse_settings(b""),
            Err((0, "version".into(), SettingsErrorKind::MissingKey))
        );
    }

    #[test]
    fn the_file_is_read_with_a_bound_and_only_if_it_is_a_regular_file() {
        let dir = temp_dir("read");
        let path = dir.join("settings.txt");
        // No file: the defaults are used.
        assert_eq!(read_settings(&path), Ok(None));
        write_settings(&path, &defaults()).expect("written");
        assert_eq!(read_settings(&path), Ok(Some(defaults())));
        assert!(!dir.join("settings.txt.tmp").exists(), "renamed into place");
        // One byte above the bound is rejected before it is parsed.
        let large = format!(
            "{}{}",
            format_settings(&defaults()),
            "#".repeat(MAX_SETTINGS_BYTES as usize)
        );
        std::fs::write(&path, &large).expect("written");
        let error = read_settings(&path).expect_err("too large");
        assert_eq!(
            error.kind,
            SettingsErrorKind::FileTooLarge {
                max_bytes: MAX_SETTINGS_BYTES
            }
        );
        let exact = &large.as_bytes()[..MAX_SETTINGS_BYTES as usize];
        std::fs::write(&path, exact).expect("written");
        assert_eq!(read_settings(&path), Ok(Some(defaults())), "at the bound");
        // A directory and a named pipe are rejected without blocking.
        std::fs::remove_file(&path).expect("removed");
        std::fs::create_dir(&path).expect("directory");
        let error = read_settings(&path).expect_err("directory");
        assert_eq!(
            error.kind,
            SettingsErrorKind::Read("not a regular file".into())
        );
        std::fs::remove_dir(&path).expect("removed");
        let made = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo runs");
        assert!(made.success());
        let (sender, receiver) = std::sync::mpsc::channel();
        let pipe = path.clone();
        std::thread::spawn(move || {
            let _ = sender.send(read_settings(&pipe));
        });
        let result = receiver
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the read returned instead of blocking");
        assert_eq!(
            result.expect_err("a pipe").kind,
            SettingsErrorKind::Read("not a regular file".into())
        );
        // The message names the file, the line, the key, and the reason.
        std::fs::remove_file(&path).expect("removed");
        std::fs::write(
            &path,
            format_settings(&defaults()).replace("scroll_speed = 5", "scroll_speed = 12"),
        )
        .expect("written");
        let message = read_settings(&path).expect_err("invalid").to_string();
        for part in [
            format!("settings file {}", path.display()),
            "line 8".into(),
            "scroll_speed".into(),
            "value \"12\" is not supported, expected 1 to 10".into(),
        ] {
            assert!(message.contains(&part), "{message} lacks {part}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn option_buttons_step_within_their_ranges_and_switch() {
        let start = defaults();
        let set = |button, settings| match options_effect(button, settings) {
            OptionsEffect::Set(next) => next,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            set(OptionsButton::Step(OptionRow::Master, true), start)
                .volume
                .master,
            9
        );
        assert_eq!(
            set(OptionsButton::Step(OptionRow::Music, false), start)
                .volume
                .music,
            4
        );
        assert_eq!(
            set(OptionsButton::Step(OptionRow::Effects, true), start)
                .volume
                .effects,
            8
        );
        assert!(
            set(OptionsButton::Switch(OptionRow::Sound), start)
                .volume
                .muted
        );
        assert!(
            set(OptionsButton::Switch(OptionRow::EdgeScrolling), start)
                .camera
                .edge_scrolling
        );
        // The ends of every range hold.
        let mut low = start;
        let mut high = start;
        for _ in 0..20 {
            for row in [
                OptionRow::Master,
                OptionRow::Music,
                OptionRow::Effects,
                OptionRow::ScrollSpeed,
            ] {
                low = set(OptionsButton::Step(row, false), low);
                high = set(OptionsButton::Step(row, true), high);
            }
        }
        assert_eq!(
            (low.volume.master, low.volume.music, low.volume.effects),
            (0, 0, 0)
        );
        assert_eq!(low.camera.scroll_speed, 1);
        assert_eq!(
            (high.volume.master, high.volume.music, high.volume.effects),
            (VOLUME_STEPS, VOLUME_STEPS, VOLUME_STEPS)
        );
        assert_eq!(high.camera.scroll_speed, SCROLL_STEPS);
        // A step on a switch, or a switch on a step, does nothing; Back closes.
        assert_eq!(
            options_effect(OptionsButton::Step(OptionRow::Sound, true), start),
            OptionsEffect::Nothing
        );
        assert_eq!(
            options_effect(OptionsButton::Switch(OptionRow::Master), start),
            OptionsEffect::Nothing
        );
        assert_eq!(
            options_effect(OptionsButton::Back, start),
            OptionsEffect::Close
        );
        // The texts and the speed factor.
        assert_eq!(option_text(OptionRow::Master, &start), "8 / 10");
        assert_eq!(option_text(OptionRow::Sound, &start), "On");
        assert_eq!(option_text(OptionRow::EdgeScrolling, &start), "Off");
        assert_eq!(option_text(OptionRow::ScrollSpeed, &start), "5 / 10");
        assert_eq!(start.camera.speed_factor(), 1.0);
        assert_eq!(low.camera.speed_factor(), 0.2);
        assert_eq!(high.camera.speed_factor(), 2.0);
        // Every row has its buttons.
        let buttons: usize = OptionRow::ALL
            .iter()
            .map(|row| if row.is_switch() { 1 } else { 2 })
            .sum::<usize>()
            + 1;
        assert_eq!(buttons, OPTIONS_BUTTONS);
    }
}
