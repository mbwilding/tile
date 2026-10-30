use std::{collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use tile_core::{LayoutKind, LayoutSettings, animation::Bezier, layout::CenterMode};

/// The whole config file. Missing sections fall back to the shipped defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub general: General,
    pub cursor: Cursor,
    pub tray: Tray,
    /// Hotkeys for launching or focusing apps and closing windows; see [`Hotkey`].
    pub hotkeys: BTreeMap<String, Hotkey>,
    pub animations: Animations,
    pub dwindle: Dwindle,
    pub master: Master,
    pub scrolling: Scrolling,
    pub remap: Remap,
    /// `"super+shift+q" = "killactive"`.
    pub binds: BTreeMap<String, String>,
    pub rules: Vec<Rule>,
}

/// What a `[hotkeys]` entry does. They work independently of tiling, and are switched on and
/// off together by the tray's Hotkey item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Hotkey {
    /// `"super+b" = "chrome.exe"`: focus a window of that program, or start it.
    Run(String),
    /// `{ run = "chrome.exe", exe = "chrome.exe" }`: start `run`, but focus a window of the
    /// process `exe` instead if there is one. `{ run = "wt.exe", focus = false }` always starts
    /// a new one.
    App {
        run: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        exe: Option<String>,
        /// Focus an existing window of the program rather than starting another. Default on.
        #[serde(default = "yes", skip_serializing_if = "is_true")]
        focus: bool,
    },
    /// `{ close = true }`: close the focused window, like Alt+F4.
    Close { close: bool },
}

impl Hotkey {
    /// The process whose window to focus rather than starting `run` again.
    pub fn exe(&self) -> Option<String> {
        match self {
            Self::Run(run) => {
                let run = run.trim();
                let program = match run.strip_prefix('"') {
                    Some(quoted) => quoted.split('"').next().unwrap_or(quoted),
                    None => run.split_whitespace().next().unwrap_or(run),
                };
                std::path::Path::new(program)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            }
            Self::App { focus: false, .. } => None,
            Self::App { exe, run, .. } => exe.clone().or_else(|| Self::Run(run.clone()).exe()),
            Self::Close { .. } => None,
        }
    }
}

fn yes() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

fn default_hotkeys() -> BTreeMap<String, Hotkey> {
    BTreeMap::from([
        (
            "super+b".to_string(),
            Hotkey::App {
                run: "chrome.exe".into(),
                exe: Some("chrome.exe".into()),
                focus: true,
            },
        ),
        (
            "super+t".to_string(),
            Hotkey::App {
                run: "wt.exe".into(),
                exe: None,
                focus: false,
            },
        ),
        ("super+q".to_string(), Hotkey::Close { close: true }),
    ])
}

/// Keybinds used when the config has no `[binds]` table.
const DEFAULT_BINDS: &[(&str, &str)] = &[
    ("super+enter", "swapwithmaster"),
    ("super+f", "togglefloating"),
    ("super+o", "fullscreen"),
    ("super+space", "cyclelayout"),
    ("super+n", "cyclenext"),
    ("super+z", "lock"),
    ("super+left", "movefocus l"),
    ("super+right", "movefocus r"),
    ("super+up", "movefocus u"),
    ("super+down", "movefocus d"),
    ("super+h", "movefocus l"),
    ("super+j", "movefocus d"),
    ("super+k", "movefocus u"),
    ("super+l", "movefocus r"), // needs `override_win_l` (the default)
    ("super+shift+left", "movewindow l"),
    ("super+shift+right", "movewindow r"),
    ("super+shift+up", "movewindow u"),
    ("super+shift+down", "movewindow d"),
    ("super+shift+h", "movewindow l"),
    ("super+shift+j", "movewindow d"),
    ("super+shift+k", "movewindow u"),
    ("super+shift+l", "movewindow r"),
    ("super+alt+left", "resizeactive -40 0"),
    ("super+alt+right", "resizeactive 40 0"),
    ("super+alt+up", "resizeactive 0 -40"),
    ("super+alt+down", "resizeactive 0 40"),
    ("super+alt+h", "resizeactive -40 0"),
    ("super+alt+j", "resizeactive 0 40"),
    ("super+alt+k", "resizeactive 0 -40"),
    ("super+alt+l", "resizeactive 40 0"),
    // Win+Ctrl+Left/Right, Win+Ctrl+D and Win+Tab are left to Windows' virtual desktops.
    ("super+ctrl+shift+left", "movetodesktop prev"),
    ("super+ctrl+shift+right", "movetodesktop next"),
    ("super+ctrl+p", "pin"),
    ("super+comma", "focusmonitor l"),
    ("super+period", "focusmonitor r"),
    ("super+s", "togglesplit"),
    ("super+r", "cycleproportion"),
    ("super+c", "centercolumn"),
    ("super+bracketleft", "consumewindow l"),
    ("super+bracketright", "consumewindow r"),
    ("super+backslash", "expelwindow"),
    ("super+shift+r", "reload"),
    ("super+shift+p", "togglepause"),
    ("super+shift+e", "exit"),
];

fn default_binds() -> BTreeMap<String, String> {
    let mut binds: BTreeMap<String, String> = DEFAULT_BINDS
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    // Workspaces 1-5 on the Dvorak home row, like the Hyprland setup; 6 and 7 on their digits.
    for (n, key) in (1..).zip(["h", "t", "n", "s", "minus"]) {
        binds.insert(format!("super+ctrl+{key}"), format!("workspace {n}"));
        binds.insert(
            format!("super+ctrl+shift+{key}"),
            format!("movetoworkspace {n}"),
        );
    }
    for n in 6..=7 {
        binds.insert(format!("super+{n}"), format!("workspace {n}"));
        binds.insert(format!("super+shift+{n}"), format!("movetoworkspace {n}"));
    }
    binds
}

/// Window rules used when the config has no `[[rules]]`.
fn default_rules() -> Vec<Rule> {
    vec![
        // Standard dialogs.
        Rule {
            class: Some("^#32770$".into()),
            float: Some(true),
            ..Rule::default()
        },
        Rule {
            class: Some(
                r"^(Windows\.UI\.Core\.CoreWindow|XamlExplorerHostIslandWindow|Shell_TrayWnd)$"
                    .into(),
            ),
            ignore: true,
            ..Rule::default()
        },
        Rule {
            exe: Some(r"^(PowerToys.*|ShareX)\.exe$".into()),
            ignore: true,
            ..Rule::default()
        },
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: General::default(),
            cursor: Cursor::default(),
            tray: Tray::default(),
            hotkeys: default_hotkeys(),
            animations: Animations::default(),
            dwindle: Dwindle::default(),
            master: Master::default(),
            scrolling: Scrolling::default(),
            remap: Remap::default(),
            binds: default_binds(),
            rules: default_rules(),
        }
    }
}

impl Config {
    pub fn layout_settings(&self) -> LayoutSettings {
        LayoutSettings {
            gaps_in: self.general.gaps_in,
            gaps_out: self.general.gaps_out,
            dwindle_split_ratio: self.dwindle.split_ratio.clamp(0.1, 0.9),
            master_mfact: self.master.mfact.clamp(0.1, 0.9),
            master_count: self.master.count.max(1),
            master_new_is_master: self.master.new_is_master,
            scrolling_proportion: self.scrolling.proportion.clamp(0.1, 1.0),
            scrolling_presets: self.scrolling.proportion_presets.clone(),
            scrolling_center: self.scrolling.center_focused,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Manage windows: layouts, `[binds]` and `[[rules]]`. Off, Tile leaves windows alone.
    pub tiling: bool,
    /// Remap keys with the `[remap]` layers.
    pub remap: bool,
    /// Space between neighbouring tiled windows.
    pub gaps_in: i32,
    /// Space between tiled windows and the monitor edge.
    pub gaps_out: i32,
    pub layout: LayoutKind,
    pub borders: bool,
    pub border_active: Color,
    pub border_inactive: Color,
    pub focus_new_windows: bool,
    /// Keep title bars on tiled windows.
    pub decorations: bool,
    /// Disable Windows' Win+L lock shortcut while Tile runs (on by default), so it can be bound.
    /// Use the `lock` dispatcher to lock instead. Restored on exit, even after a crash.
    pub override_win_l: bool,
    /// Switch off Windows' Click to Do (Copilot+ PCs): holding Win would otherwise glow around
    /// the pointer. It stays off after Tile exits; turn it back on in Settings.
    pub disable_click_to_do: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            tiling: true,
            remap: true,
            gaps_in: 10,
            gaps_out: 10,
            layout: LayoutKind::Master,
            borders: true,
            border_active: Color::rgb(0x89, 0xb4, 0xfa),
            border_inactive: Color::rgb(0x45, 0x47, 0x5a),
            focus_new_windows: true,
            decorations: false,
            override_win_l: true,
            disable_click_to_do: true,
        }
    }
}

/// The tray's switches, saved here whenever they are changed so they survive a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tray {
    /// Window management is on. Off until switched on.
    pub tile: bool,
    /// Key remapping is on. Off until switched on.
    pub key: bool,
    /// The `[hotkeys]` are on.
    pub hotkey: bool,
    /// Logging to the log file is on.
    pub logging: bool,
}

impl Default for Tray {
    fn default() -> Self {
        Self {
            tile: false,
            key: false,
            hotkey: true,
            logging: false,
        }
    }
}

/// The mouse pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cursor {
    /// Move the pointer onto the focused window (the middle of the monitor, if it is empty)
    /// whenever a keybind or dispatch moves focus, then hide it like typing does, until the
    /// mouse moves. It stays put if it is already over the window.
    pub warp: bool,
    /// Hide the pointer when a key is typed, until the mouse moves or clicks, or another
    /// monitor is focused. Modifiers on their own and keybinds don't count as typing.
    pub hide_on_key: bool,
    /// Hold Win and drag a window with the left button, from anywhere on it.
    pub drag: bool,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            warp: true,
            hide_on_key: true,
            drag: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Animations {
    pub enabled: bool,
    pub duration_ms: u64,
    pub bezier: [f32; 4],
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_ms: 180,
            bezier: [0.0, 0.75, 0.15, 1.0],
        }
    }
}

impl Animations {
    pub const fn curve(&self) -> Bezier {
        let [x1, y1, x2, y2] = self.bezier;
        Bezier::new(x1, y1, x2, y2)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dwindle {
    pub split_ratio: f32,
}

impl Default for Dwindle {
    fn default() -> Self {
        Self { split_ratio: 0.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Master {
    pub mfact: f32,
    pub count: usize,
    pub new_is_master: bool,
}

impl Default for Master {
    fn default() -> Self {
        Self {
            mfact: 0.5,
            count: 1,
            new_is_master: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scrolling {
    pub proportion: f32,
    pub proportion_presets: Vec<f32>,
    pub center_focused: CenterMode,
}

impl Default for Scrolling {
    fn default() -> Self {
        Self {
            proportion: 0.333,
            proportion_presets: vec![0.25, 0.333, 0.5, 1.0],
            center_focused: CenterMode::OnOverflow,
        }
    }
}

/// Key remapping, keyd style. Each `layers` entry is a layer named as a keyd section header:
/// `main`, `name`, `name:C` (a layer whose unbound keys get Control) or `a+b` (while both
/// are active). Its table maps keys to actions, e.g. `capslock = "overload(nav, esc)"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Remap {
    /// Tap a dummy key before a layer's lone Alt or Win is released, so it doesn't open a menu
    /// or Start (keyd's modifier guard).
    pub modifier_guard: bool,
    pub layers: BTreeMap<String, BTreeMap<String, String>>,
}

/// Remapping layers used when the config has no `[remap.layers]`: nona's keyd setup (the
/// `keyd-dvorak` module in the nix repo), so the keyboard behaves the same on Linux. Its
/// `rightalt = layer(symbols)` waits for a port of the `symbols` layer.
const DEFAULT_LAYERS: &[(&str, &[(&str, &str)])] = &[
    (
        "main",
        &[
            ("backspace", "noop"),
            ("space", "overload(shift, space)"),
            ("capslock", "overload(capslock, backspace)"),
            ("leftshift", "esc"),
            ("a", "overloadt(control, a, 200)"),
            (";", "overloadt(control, ;, 200)"),
            ("s", "overloadt(meta, s, 200)"),
            ("l", "overloadt(meta, l, 200)"),
            ("z", "overloadt(alt, z, 200)"),
            ("/", "overloadt(alt, /, 200)"),
            ("f", "overloadt(numbers, f, 200)"),
            ("m", "overloadt(fkeys, m, 200)"),
        ],
    ),
    // With the Windows Dvorak layout, j c v p are Dvorak's h j k l.
    (
        "capslock:C",
        &[("j", "left"), ("c", "down"), ("v", "up"), ("p", "right")],
    ),
    (
        "capslock+shift",
        &[
            ("j", "C-left"),
            ("c", "C-down"),
            ("v", "C-up"),
            ("p", "C-right"),
        ],
    ),
    (
        "numbers",
        &[
            (";", "0"),
            ("m", "1"),
            (",", "2"),
            (".", "3"),
            ("j", "4"),
            ("k", "5"),
            ("l", "6"),
            ("u", "7"),
            ("i", "8"),
            ("o", "9"),
        ],
    ),
    (
        "fkeys",
        &[
            ("q", "f1"),
            ("w", "f2"),
            ("e", "f3"),
            ("r", "f4"),
            ("a", "f5"),
            ("s", "f6"),
            ("d", "f7"),
            ("f", "f8"),
            ("z", "f9"),
            ("x", "f10"),
            ("c", "f11"),
            ("v", "f12"),
        ],
    ),
];

impl Default for Remap {
    fn default() -> Self {
        let layers = DEFAULT_LAYERS
            .iter()
            .map(|(name, binds)| {
                let binds = binds
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                    .collect();
                ((*name).to_string(), binds)
            })
            .collect();
        Self {
            modifier_guard: true,
            layers,
        }
    }
}

/// A window rule. All given matchers must match for the actions to apply.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    pub exe: Option<String>,
    pub class: Option<String>,
    pub title: Option<String>,
    pub float: Option<bool>,
    pub workspace: Option<u8>,
    pub monitor: Option<usize>,
    /// Force the title bar on or off for matching windows, tiled or floating.
    pub decorations: Option<bool>,
    pub ignore: bool,
}

/// An sRGB color written as `#rrggbb`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.trim().trim_start_matches('#');
        let channel = |i: usize| {
            hex.get(i..i + 2)
                .and_then(|c| u8::from_str_radix(c, 16).ok())
                .ok_or_else(|| format!("invalid color `{s}`, expected #rrggbb"))
        };
        if hex.len() != 6 {
            return Err(format!("invalid color `{s}`, expected #rrggbb"));
        }
        Ok(Self::rgb(channel(0)?, channel(2)?, channel(4)?))
    }
}

impl TryFrom<String> for Color {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Color> for String {
    fn from(color: Color) -> Self {
        color.to_string()
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}
