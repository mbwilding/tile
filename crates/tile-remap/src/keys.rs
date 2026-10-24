use std::fmt;

/// A physical key: its PC scancode (set 1), with `0xE0` in the high byte for extended keys.
///
/// Scancodes name positions, not characters, so (as in keyd) `j` means the key where QWERTY
/// has `J`, whatever layout Windows is set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(u16);

const EXT: u16 = 0xE000;

impl Key {
    pub const ESC: Self = Self(0x01);
    pub const BACKSPACE: Self = Self(0x0E);
    pub const ENTER: Self = Self(0x1C);
    pub const LEFTCONTROL: Self = Self(0x1D);
    pub const LEFTSHIFT: Self = Self(0x2A);
    pub const LEFTALT: Self = Self(0x38);
    pub const RIGHTALT: Self = Self(EXT | 0x38);
    pub const LEFTMETA: Self = Self(EXT | 0x5B);

    pub const fn new(scan: u8, extended: bool) -> Self {
        Self(scan as u16 | if extended { EXT } else { 0 })
    }

    pub const fn scan(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    pub const fn extended(self) -> bool {
        self.0 & EXT != 0
    }

    /// Looks up a keyd key name, e.g. `a`, `;`, `leftshift` or `esc`.
    pub fn from_name(name: &str) -> Option<Self> {
        NAMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&(_, code)| Self(code))
    }

    /// The canonical keyd name, if the key has one.
    pub fn name(self) -> Option<&'static str> {
        NAMES
            .iter()
            .find(|&&(_, code)| code == self.0)
            .map(|&(n, _)| n)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "{:#06x}", self.0),
        }
    }
}

/// keyd key names and their scancodes. The first name for a code is its canonical one.
const NAMES: &[(&str, u16)] = &[
    ("esc", 0x01),
    ("escape", 0x01),
    ("1", 0x02),
    ("2", 0x03),
    ("3", 0x04),
    ("4", 0x05),
    ("5", 0x06),
    ("6", 0x07),
    ("7", 0x08),
    ("8", 0x09),
    ("9", 0x0A),
    ("0", 0x0B),
    ("minus", 0x0C),
    ("-", 0x0C),
    ("equal", 0x0D),
    ("=", 0x0D),
    ("backspace", 0x0E),
    ("tab", 0x0F),
    ("q", 0x10),
    ("w", 0x11),
    ("e", 0x12),
    ("r", 0x13),
    ("t", 0x14),
    ("y", 0x15),
    ("u", 0x16),
    ("i", 0x17),
    ("o", 0x18),
    ("p", 0x19),
    ("leftbrace", 0x1A),
    ("[", 0x1A),
    ("rightbrace", 0x1B),
    ("]", 0x1B),
    ("enter", 0x1C),
    ("leftcontrol", 0x1D),
    ("a", 0x1E),
    ("s", 0x1F),
    ("d", 0x20),
    ("f", 0x21),
    ("g", 0x22),
    ("h", 0x23),
    ("j", 0x24),
    ("k", 0x25),
    ("l", 0x26),
    ("semicolon", 0x27),
    (";", 0x27),
    ("apostrophe", 0x28),
    ("'", 0x28),
    ("grave", 0x29),
    ("`", 0x29),
    ("leftshift", 0x2A),
    ("backslash", 0x2B),
    ("\\", 0x2B),
    ("z", 0x2C),
    ("x", 0x2D),
    ("c", 0x2E),
    ("v", 0x2F),
    ("b", 0x30),
    ("n", 0x31),
    ("m", 0x32),
    ("comma", 0x33),
    (",", 0x33),
    ("dot", 0x34),
    (".", 0x34),
    ("slash", 0x35),
    ("/", 0x35),
    ("rightshift", 0x36),
    ("kpasterisk", 0x37),
    ("leftalt", 0x38),
    ("space", 0x39),
    ("capslock", 0x3A),
    ("f1", 0x3B),
    ("f2", 0x3C),
    ("f3", 0x3D),
    ("f4", 0x3E),
    ("f5", 0x3F),
    ("f6", 0x40),
    ("f7", 0x41),
    ("f8", 0x42),
    ("f9", 0x43),
    ("f10", 0x44),
    ("scrolllock", 0x46),
    ("kp7", 0x47),
    ("kp8", 0x48),
    ("kp9", 0x49),
    ("kpminus", 0x4A),
    ("kp4", 0x4B),
    ("kp5", 0x4C),
    ("kp6", 0x4D),
    ("kpplus", 0x4E),
    ("kp1", 0x4F),
    ("kp2", 0x50),
    ("kp3", 0x51),
    ("kp0", 0x52),
    ("kpdot", 0x53),
    ("102nd", 0x56),
    ("f11", 0x57),
    ("f12", 0x58),
    ("f13", 0x64),
    ("f14", 0x65),
    ("f15", 0x66),
    ("f16", 0x67),
    ("f17", 0x68),
    ("f18", 0x69),
    ("f19", 0x6A),
    ("f20", 0x6B),
    ("f21", 0x6C),
    ("f22", 0x6D),
    ("f23", 0x6E),
    ("f24", 0x76),
    ("previoussong", EXT | 0x10),
    ("nextsong", EXT | 0x19),
    ("kpenter", EXT | 0x1C),
    ("rightcontrol", EXT | 0x1D),
    ("mute", EXT | 0x20),
    ("playpause", EXT | 0x22),
    ("stopcd", EXT | 0x24),
    ("volumedown", EXT | 0x2E),
    ("volumeup", EXT | 0x30),
    ("kpslash", EXT | 0x35),
    ("sysrq", EXT | 0x37),
    ("print", EXT | 0x37),
    ("rightalt", EXT | 0x38),
    ("numlock", EXT | 0x45),
    ("home", EXT | 0x47),
    ("up", EXT | 0x48),
    ("pageup", EXT | 0x49),
    ("left", EXT | 0x4B),
    ("right", EXT | 0x4D),
    ("end", EXT | 0x4F),
    ("down", EXT | 0x50),
    ("pagedown", EXT | 0x51),
    ("insert", EXT | 0x52),
    ("delete", EXT | 0x53),
    ("leftmeta", EXT | 0x5B),
    ("rightmeta", EXT | 0x5C),
    ("compose", EXT | 0x5D),
    ("menu", EXT | 0x5D),
];

/// Characters keyd accepts as shorthand for a shifted key, e.g. `!` for `S-1` (US positions).
pub fn shifted(c: char) -> Option<Key> {
    let base = match c {
        '!' => "1",
        '@' => "2",
        '#' => "3",
        '$' => "4",
        '%' => "5",
        '^' => "6",
        '&' => "7",
        '*' => "8",
        '(' => "9",
        ')' => "0",
        '_' => "minus",
        '+' => "equal",
        '{' => "leftbrace",
        '}' => "rightbrace",
        '|' => "backslash",
        ':' => "semicolon",
        '"' => "apostrophe",
        '<' => "comma",
        '>' => "dot",
        '?' => "slash",
        '~' => "grave",
        'A'..='Z' => return Key::from_name(&c.to_ascii_lowercase().to_string()),
        _ => return None,
    };
    Key::from_name(base)
}

/// Modifiers held by a layer or added by a key sequence such as `C-left`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Mods(u8);

impl Mods {
    pub const NONE: Self = Self(0);
    pub const CONTROL: Self = Self(1);
    pub const SHIFT: Self = Self(2);
    pub const ALT: Self = Self(4);
    pub const META: Self = Self(8);
    pub const ALTGR: Self = Self(16);

    /// Each modifier with the key that produces it, in the order they are pressed.
    pub(crate) const KEYS: [(Self, Key); 5] = [
        (Self::CONTROL, Key::LEFTCONTROL),
        (Self::SHIFT, Key::LEFTSHIFT),
        (Self::ALT, Key::LEFTALT),
        (Self::META, Key::LEFTMETA),
        (Self::ALTGR, Key::RIGHTALT),
    ];

    /// keyd's modifier letters: `C`, `S`, `A`, `M` and `G`.
    pub fn from_letter(c: char) -> Option<Self> {
        match c {
            'C' => Some(Self::CONTROL),
            'S' => Some(Self::SHIFT),
            'A' => Some(Self::ALT),
            'M' => Some(Self::META),
            'G' => Some(Self::ALTGR),
            _ => None,
        }
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Mods {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for Mods {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_to_canonical() {
        assert_eq!(Key::from_name(";"), Key::from_name("semicolon"));
        assert_eq!(Key::from_name("escape").unwrap().name(), Some("esc"));
        assert_eq!(Key::from_name("leftshift"), Some(Key::LEFTSHIFT));
        // Uppercase letters are shifted keys (`A` is `S-a`), not names.
        assert_eq!(Key::from_name("A"), None);
        let left = Key::from_name("left").unwrap();
        assert_eq!((left.scan(), left.extended()), (0x4B, true));
        assert_eq!(Key::from_name("nope"), None);
    }

    #[test]
    fn shifted_characters() {
        assert_eq!(shifted('?'), Key::from_name("/"));
        assert_eq!(shifted('Q'), Key::from_name("q"));
        assert_eq!(shifted('q'), None);
    }
}
