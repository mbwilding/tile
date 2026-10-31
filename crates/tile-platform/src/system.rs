//! Reading the system for the launcher's system items and carrying out their actions.

use tile_core::system::{Action, Battery, Network, Radio, Status, Volume, format_clock};
use tile_hook::system as sys;
use winsafe as w;

fn radio_kind(radio: Radio) -> sys::RadioKind {
    match radio {
        Radio::Wifi => sys::RadioKind::Wifi,
        Radio::Bluetooth => sys::RadioKind::Bluetooth,
    }
}

/// The local time and date, like `10:42 pm, Saturday 31 October 2026`.
fn clock() -> String {
    let t = w::GetLocalTime();
    format_clock(t.wYear, t.wMonth, t.wDay, t.wDayOfWeek, t.wHour, t.wMinute)
}

/// Reads the volume, battery, network and radios. Some calls wait on Windows, so call this off
/// the UI thread.
pub fn status() -> Status {
    Status {
        volume: sys::volume().map(|v| Volume {
            percent: v.percent,
            muted: v.muted,
        }),
        battery: sys::battery().map(|b| Battery {
            percent: b.percent,
            charging: b.charging,
            plugged_in: b.plugged_in,
            minutes_left: b.minutes_left,
        }),
        network: sys::network().map(|n| Network {
            name: n.name,
            internet: n.internet,
        }),
        wifi: sys::radio(sys::RadioKind::Wifi),
        bluetooth: sys::radio(sys::RadioKind::Bluetooth),
        clock: Some(clock()),
    }
}

/// Does what the audio and radio actions say; blocks like [`status`]. The other actions start
/// something and are the caller's to run (see [`command_line`]). Returns whether it worked.
pub fn apply(action: Action) -> bool {
    match action {
        Action::SetVolume(percent) => sys::set_volume(percent),
        Action::Mute(muted) => sys::set_muted(muted),
        Action::SetRadio(radio, on) => sys::set_radio(radio_kind(radio), on),
        Action::Open(_) | Action::Power(_) => false,
    }
}

/// The command line that carries out an action that starts a program, if it is one.
pub fn command_line(action: Action) -> Option<String> {
    match action {
        Action::Open(uri) => Some(uri.to_owned()),
        Action::Power(power) => Some(power.command_line().to_owned()),
        Action::SetVolume(_) | Action::Mute(_) | Action::SetRadio(..) => None,
    }
}
