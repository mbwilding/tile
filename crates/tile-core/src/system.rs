//! The launcher's system items: the volume, network, Wi-Fi, Bluetooth, battery, clock and
//! power actions that Explorer's taskbar would otherwise offer. This builds the rows from a
//! snapshot of the system and searches them; reading and changing the system is the platform's.

use crate::search::fuzzy;

/// The default output's volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volume {
    /// 0 to 100.
    pub percent: u8,
    pub muted: bool,
}

/// The battery of a laptop or tablet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub plugged_in: bool,
    /// On battery power: the estimated minutes left.
    pub minutes_left: Option<u32>,
}

/// The network connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    pub name: Option<String>,
    pub internet: bool,
}

/// A radio that can be switched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Radio {
    Wifi,
    Bluetooth,
}

/// Ending the session or putting the machine away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    Lock,
    SwitchUser,
    Sleep,
    SignOut,
    Restart,
    ShutDown,
}

impl Power {
    /// Whether the launcher asks to press Enter again, because it would lose unsaved work.
    pub const fn needs_confirmation(self) -> bool {
        matches!(self, Self::SignOut | Self::Restart | Self::ShutDown)
    }

    /// The command line that does it.
    pub const fn command_line(self) -> &'static str {
        match self {
            Self::Lock => "rundll32.exe user32.dll,LockWorkStation",
            // Leaves the session running and shows the sign-in screen.
            Self::SwitchUser => "tsdisconnect.exe",
            Self::Sleep => "rundll32.exe powrprof.dll,SetSuspendState 0,1,0",
            Self::SignOut => "shutdown.exe /l",
            Self::Restart => "shutdown.exe /r /t 0",
            Self::ShutDown => "shutdown.exe /s /t 0",
        }
    }
}

/// What picking a system row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    SetVolume(u8),
    Mute(bool),
    SetRadio(Radio, bool),
    /// Open a Settings page (an `ms-settings:` URI).
    Open(&'static str),
    Power(Power),
}

/// What the system is doing, as far as it could be read. Whatever is missing (no battery, no
/// Bluetooth) has no rows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    pub volume: Option<Volume>,
    pub battery: Option<Battery>,
    pub network: Option<Network>,
    pub wifi: Option<bool>,
    pub bluetooth: Option<bool>,
    /// The time and date, ready to show.
    pub clock: Option<String>,
}

/// A row: what it is called, what it says underneath, and what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    pub subtitle: String,
    /// Other words that find it.
    pub keywords: &'static str,
    pub action: Action,
}

fn entry(
    title: &str,
    subtitle: impl Into<String>,
    keywords: &'static str,
    action: Action,
) -> Entry {
    Entry {
        title: title.into(),
        subtitle: subtitle.into(),
        keywords,
        action,
    }
}

const VOLUME_STEP: u8 = 10;

fn radio_entry(
    title: &str,
    keywords: &'static str,
    radio: Radio,
    on: Option<bool>,
) -> Option<Entry> {
    let on = on?;
    let (state, hint) = if on {
        ("On", "Enter to turn off")
    } else {
        ("Off", "Enter to turn on")
    };
    Some(entry(
        title,
        format!("{state}. {hint}"),
        keywords,
        Action::SetRadio(radio, !on),
    ))
}

fn battery_subtitle(b: &Battery) -> String {
    if b.charging {
        "Charging".to_owned()
    } else if b.plugged_in {
        "Plugged in".to_owned()
    } else if let Some(minutes) = b.minutes_left {
        format!("On battery, {}h {:02}m left", minutes / 60, minutes % 60)
    } else {
        "On battery".to_owned()
    }
}

/// Every row for `status`, in the order they are listed.
pub fn entries(status: &Status) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Some(v) = status.volume {
        let (title, hint) = if v.muted {
            ("Volume: muted".to_owned(), "Enter to unmute")
        } else {
            (format!("Volume: {}%", v.percent), "Enter to mute")
        };
        out.push(entry(
            &title,
            hint,
            "sound audio speakers mute unmute",
            Action::Mute(!v.muted),
        ));
        out.push(entry(
            "Volume up",
            format!("To {}%", v.percent.saturating_add(VOLUME_STEP).min(100)),
            "louder sound audio",
            Action::SetVolume(v.percent.saturating_add(VOLUME_STEP).min(100)),
        ));
        out.push(entry(
            "Volume down",
            format!("To {}%", v.percent.saturating_sub(VOLUME_STEP)),
            "quieter sound audio",
            Action::SetVolume(v.percent.saturating_sub(VOLUME_STEP)),
        ));
    }
    if let Some(n) = &status.network {
        let title = match (&n.name, n.internet) {
            (Some(name), _) => format!("Network: {name}"),
            (None, true) => "Network: connected".to_owned(),
            (None, false) => "Network: not connected".to_owned(),
        };
        let subtitle = if n.internet {
            "Connected to the internet. Enter for network settings"
        } else {
            "No internet. Enter for network settings"
        };
        out.push(entry(
            &title,
            subtitle,
            "internet wifi wi-fi ethernet connection",
            Action::Open("ms-settings:network"),
        ));
    }
    out.extend(radio_entry(
        "Wi-Fi",
        "wifi wireless network",
        Radio::Wifi,
        status.wifi,
    ));
    out.extend(radio_entry(
        "Bluetooth",
        "wireless devices",
        Radio::Bluetooth,
        status.bluetooth,
    ));
    if let Some(b) = &status.battery {
        out.push(entry(
            &format!("Battery: {}%", b.percent),
            format!("{}. Enter for battery settings", battery_subtitle(b)),
            "power charge charging",
            Action::Open("ms-settings:batterysaver"),
        ));
    }
    if let Some(clock) = &status.clock {
        out.push(entry(
            "Clock",
            format!("{clock}. Enter for date and time settings"),
            "time date calendar",
            Action::Open("ms-settings:dateandtime"),
        ));
    }
    for (title, keywords, power) in [
        ("Lock", "lock screen", Power::Lock),
        ("Switch user", "change account", Power::SwitchUser),
        ("Sleep", "suspend", Power::Sleep),
        ("Sign out", "log off logout", Power::SignOut),
        ("Restart", "reboot", Power::Restart),
        ("Shut down", "power off turn off", Power::ShutDown),
    ] {
        let subtitle = if power.needs_confirmation() {
            "Power. Enter twice to confirm"
        } else {
            "Power"
        };
        out.push(entry(title, subtitle, keywords, Action::Power(power)));
    }
    out
}

/// `volume 30` or `vol 30%`: the percentage asked for.
fn volume_request(query: &str) -> Option<u8> {
    let (word, number) = query.trim().split_once(char::is_whitespace)?;
    if !["volume", "vol"].contains(&word.to_lowercase().as_str()) {
        return None;
    }
    let percent: u8 = number.trim().trim_end_matches('%').parse().ok()?;
    (percent <= 100).then_some(percent)
}

/// The rows for `status` that match `query`, best first. An empty query matches them all.
pub fn find(query: &str, status: &Status) -> Vec<Entry> {
    let query = query.trim();
    let mut all = entries(status);
    let mut out = Vec::new();
    if status.volume.is_some()
        && let Some(percent) = volume_request(query)
    {
        out.push(entry(
            &format!("Set volume to {percent}%"),
            "Enter to set it",
            "",
            Action::SetVolume(percent),
        ));
    }
    if query.is_empty() {
        out.append(&mut all);
        return out;
    }
    let mut scored: Vec<(f64, Entry)> = all
        .into_iter()
        .filter_map(|e| {
            let by_title = fuzzy(query, &e.title);
            let by_keywords = fuzzy(query, e.keywords).map(|s| s * 0.5);
            by_title
                .into_iter()
                .chain(by_keywords)
                .max_by(f64::total_cmp)
                .map(|score| (score, e))
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.extend(scored.into_iter().map(|(_, e)| e));
    out
}

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// `10:42 pm, Saturday 31 October 2026`. `weekday` counts from Sunday = 0, `month` from 1.
pub fn format_clock(
    year: u16,
    month: u16,
    day: u16,
    weekday: u16,
    hour: u16,
    minute: u16,
) -> String {
    let (hour12, suffix) = match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
    };
    format!(
        "{hour12}:{minute:02} {suffix}, {} {day} {} {year}",
        WEEKDAYS[usize::from(weekday) % 7],
        MONTHS[usize::from(month.saturating_sub(1)) % 12],
    )
}

/// `10:42:07 pm`.
pub fn format_time(hour: u16, minute: u16, second: u16) -> String {
    let (hour12, suffix) = match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
    };
    format!("{hour12}:{minute:02}:{second:02} {suffix}")
}

/// `Saturday 31 October 2026`. `weekday` counts from Sunday = 0, `month` from 1.
pub fn format_date(year: u16, month: u16, day: u16, weekday: u16) -> String {
    format!(
        "{} {day} {} {year}",
        WEEKDAYS[usize::from(weekday) % 7],
        MONTHS[usize::from(month.saturating_sub(1)) % 12],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Status {
        Status {
            volume: Some(Volume {
                percent: 45,
                muted: false,
            }),
            battery: Some(Battery {
                percent: 80,
                charging: false,
                plugged_in: false,
                minutes_left: Some(125),
            }),
            network: Some(Network {
                name: Some("Home".into()),
                internet: true,
            }),
            wifi: Some(true),
            bluetooth: Some(false),
            clock: Some("10:42 pm, Saturday 31 October 2026".into()),
        }
    }

    fn titles(query: &str, status: &Status) -> Vec<String> {
        find(query, status).into_iter().map(|e| e.title).collect()
    }

    #[test]
    fn missing_hardware_has_no_rows() {
        let all = titles("", &Status::default());
        assert_eq!(
            all,
            [
                "Lock",
                "Switch user",
                "Sleep",
                "Sign out",
                "Restart",
                "Shut down"
            ]
        );
    }

    #[test]
    fn every_row_is_listed_for_an_empty_query() {
        let all = titles("", &status());
        for title in [
            "Volume: 45%",
            "Network: Home",
            "Wi-Fi",
            "Bluetooth",
            "Battery: 80%",
            "Clock",
        ] {
            assert!(
                all.iter().any(|t| t == title),
                "{title} is missing from {all:?}"
            );
        }
    }

    #[test]
    fn toggles_flip_the_current_state() {
        let rows = find("bluetooth", &status());
        assert_eq!(rows[0].action, Action::SetRadio(Radio::Bluetooth, true));
        let rows = find("wi-fi", &status());
        assert_eq!(rows[0].action, Action::SetRadio(Radio::Wifi, false));
        let rows = find("mute", &status());
        assert_eq!(rows[0].action, Action::Mute(true));
    }

    #[test]
    fn keywords_find_rows() {
        assert_eq!(titles("reboot", &status())[0], "Restart");
        assert_eq!(titles("logout", &status())[0], "Sign out");
    }

    #[test]
    fn volume_can_be_typed() {
        let rows = find("volume 30", &status());
        assert_eq!(rows[0].action, Action::SetVolume(30));
        assert_eq!(rows[0].title, "Set volume to 30%");
        assert!(
            find("vol 101%", &status())
                .iter()
                .all(|e| !e.title.starts_with("Set volume"))
        );
        assert!(volume_request("volume").is_none());
    }

    #[test]
    fn volume_steps_stay_in_range() {
        let loud = Status {
            volume: Some(Volume {
                percent: 98,
                muted: false,
            }),
            ..Status::default()
        };
        let up = find("volume up", &loud);
        assert_eq!(up[0].action, Action::SetVolume(100));
        let quiet = Status {
            volume: Some(Volume {
                percent: 4,
                muted: true,
            }),
            ..Status::default()
        };
        assert_eq!(find("volume down", &quiet)[0].action, Action::SetVolume(0));
    }

    #[test]
    fn only_data_losing_power_actions_ask_first() {
        assert!(!Power::Lock.needs_confirmation());
        assert!(!Power::Sleep.needs_confirmation());
        assert!(!Power::SwitchUser.needs_confirmation());
        assert!(Power::ShutDown.needs_confirmation());
        assert!(Power::Restart.needs_confirmation());
        assert!(Power::SignOut.needs_confirmation());
    }

    #[test]
    fn formats_the_time_with_seconds_and_the_date() {
        assert_eq!(format_time(22, 42, 7), "10:42:07 pm");
        assert_eq!(format_time(0, 5, 0), "12:05:00 am");
        assert_eq!(format_time(12, 0, 59), "12:00:59 pm");
        assert_eq!(format_date(2026, 10, 31, 6), "Saturday 31 October 2026");
    }

    #[test]
    fn formats_the_clock() {
        assert_eq!(
            format_clock(2026, 10, 31, 6, 22, 42),
            "10:42 pm, Saturday 31 October 2026"
        );
        assert_eq!(
            format_clock(2026, 1, 1, 4, 0, 5),
            "12:05 am, Thursday 1 January 2026"
        );
        assert_eq!(
            format_clock(2026, 1, 1, 4, 12, 0),
            "12:00 pm, Thursday 1 January 2026"
        );
    }
}
