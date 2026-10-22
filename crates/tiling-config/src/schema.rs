use std::{collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use tiling_core::{
    LayoutKind, LayoutSettings, animation::Bezier, layout::CenterMode,
};

/// The whole config file. Missing sections fall back to the shipped defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub general: General,
    pub animations: Animations,
    pub dwindle: Dwindle,
    pub master: Master,
    pub scrolling: Scrolling,
    /// `"super+shift+q" = "killactive"`.
    pub binds: BTreeMap<String, String>,
    pub rules: Vec<Rule>,
}

/// The binds and rules from the shipped config, which are too long to duplicate in code.
#[derive(Deserialize)]
struct Bundled {
    binds: BTreeMap<String, String>,
    rules: Vec<Rule>,
}

impl Default for Config {
    fn default() -> Self {
        let bundled: Bundled =
            toml::from_str(crate::DEFAULT_TOML).expect("bundled default config is valid");
        Self {
            general: General::default(),
            animations: Animations::default(),
            dwindle: Dwindle::default(),
            master: Master::default(),
            scrolling: Scrolling::default(),
            binds: bundled.binds,
            rules: bundled.rules,
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
    pub gaps_in: i32,
    pub gaps_out: i32,
    pub layout: LayoutKind,
    pub borders: bool,
    pub border_active: Color,
    pub border_inactive: Color,
    pub focus_new_windows: bool,
    /// Keep title bars on tiled windows.
    pub decorations: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            gaps_in: 5,
            gaps_out: 10,
            layout: LayoutKind::Dwindle,
            borders: true,
            border_active: Color::rgb(0x89, 0xb4, 0xfa),
            border_inactive: Color::rgb(0x45, 0x47, 0x5a),
            focus_new_windows: true,
            decorations: true,
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
        Self { enabled: true, duration_ms: 150, bezier: [0.05, 0.9, 0.1, 1.0] }
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
        Self { mfact: 0.55, count: 1, new_is_master: false }
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
            proportion: 0.5,
            proportion_presets: vec![0.333, 0.5, 0.667, 1.0],
            center_focused: CenterMode::OnOverflow,
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
