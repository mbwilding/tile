//! What the launcher shows and changes for the system tray items Explorer would otherwise
//! provide: the volume, the battery, the network and the Wi-Fi and Bluetooth radios.
//!
//! Each call initialises COM on the calling thread for its own duration, so call them from a
//! worker thread, not one with a message loop.

use windows::{
    Devices::Radios::{Radio, RadioAccessStatus, RadioKind as WinRadioKind, RadioState},
    Win32::{
        Media::Audio::{
            Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole,
            eRender,
        },
        Networking::NetworkListManager::{
            INetwork, INetworkListManager, NLM_CONNECTIVITY_IPV4_INTERNET,
            NLM_CONNECTIVITY_IPV6_INTERNET, NLM_ENUM_NETWORK_CONNECTED, NetworkListManager,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
        },
    },
};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

/// Keeps COM initialised on this thread until dropped.
struct Com(bool);

impl Com {
    fn init() -> Self {
        // SAFETY: takes no pointers. Initialising can fail if the thread is already in another
        // apartment mode, which is fine to run in; then there is nothing to undo.
        Self(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() })
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx in `init`.
            unsafe { CoUninitialize() };
        }
    }
}

/// The default speakers' volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Volume {
    /// 0 to 100.
    pub percent: u8,
    pub muted: bool,
}

fn endpoint() -> Option<IAudioEndpointVolume> {
    // SAFETY: plain COM calls on interfaces this function owns; COM is initialised by the
    // caller's `Com` guard.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        device
            .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
            .ok()
    }
}

/// The volume and mute state of the default output, if there is one.
pub fn volume() -> Option<Volume> {
    let _com = Com::init();
    let endpoint = endpoint()?;
    // SAFETY: plain COM calls on the interface above.
    unsafe {
        let level = endpoint.GetMasterVolumeLevelScalar().ok()?;
        let muted = endpoint.GetMute().ok()?.as_bool();
        Some(Volume {
            percent: (level.clamp(0.0, 1.0) * 100.0).round() as u8,
            muted,
        })
    }
}

/// Sets the default output's volume, 0 to 100, and unmutes it.
pub fn set_volume(percent: u8) -> bool {
    let _com = Com::init();
    let Some(endpoint) = endpoint() else {
        return false;
    };
    let level = f32::from(percent.min(100)) / 100.0;
    // SAFETY: plain COM calls on the interface above; a null event context is allowed.
    unsafe {
        endpoint
            .SetMasterVolumeLevelScalar(level, std::ptr::null())
            .is_ok()
            && endpoint.SetMute(false, std::ptr::null()).is_ok()
    }
}

/// Mutes or unmutes the default output.
pub fn set_muted(muted: bool) -> bool {
    let _com = Com::init();
    let Some(endpoint) = endpoint() else {
        return false;
    };
    // SAFETY: a plain COM call on the interface above; a null event context is allowed.
    unsafe { endpoint.SetMute(muted, std::ptr::null()).is_ok() }
}

/// The battery, on a machine that has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
    pub plugged_in: bool,
    /// On battery power: the minutes the system estimates are left.
    pub minutes_left: Option<u32>,
}

pub fn battery() -> Option<Battery> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a live SYSTEM_POWER_STATUS to write to.
    if unsafe { GetSystemPowerStatus(&raw mut status) } == 0 {
        return None;
    }
    const NO_BATTERY: u8 = 128;
    const CHARGING: u8 = 8;
    const UNKNOWN: u8 = 255;
    if status.BatteryFlag == UNKNOWN || status.BatteryFlag & NO_BATTERY != 0 {
        return None;
    }
    let plugged_in = status.ACLineStatus == 1;
    Some(Battery {
        percent: status.BatteryLifePercent.min(100),
        charging: status.BatteryFlag & CHARGING != 0,
        plugged_in,
        minutes_left: (!plugged_in && status.BatteryLifeTime != u32::MAX)
            .then_some(status.BatteryLifeTime / 60),
    })
}

/// The network connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    /// The connected network's name, like the Wi-Fi network.
    pub name: Option<String>,
    pub internet: bool,
}

pub fn network() -> Option<Network> {
    let _com = Com::init();
    // SAFETY: plain COM calls on interfaces this function owns; `fetched` and `slot` are live
    // locals the calls write to.
    unsafe {
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok()?;
        let connectivity = manager.GetConnectivity().ok()?.0;
        let internet = connectivity
            & (NLM_CONNECTIVITY_IPV4_INTERNET.0 | NLM_CONNECTIVITY_IPV6_INTERNET.0)
            != 0;
        let networks = manager.GetNetworks(NLM_ENUM_NETWORK_CONNECTED).ok()?;
        let mut name = None;
        loop {
            let mut slot: [Option<INetwork>; 1] = [None];
            let mut fetched = 0;
            if networks.Next(&mut slot, Some(&raw mut fetched)).is_err() || fetched == 0 {
                break;
            }
            if let Some(network) = slot[0].take()
                && let Ok(found) = network.GetName()
            {
                name = Some(found.to_string());
                break;
            }
        }
        Some(Network { name, internet })
    }
}

/// A radio the system can switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioKind {
    Wifi,
    Bluetooth,
}

impl RadioKind {
    fn windows(self) -> WinRadioKind {
        match self {
            Self::Wifi => WinRadioKind::WiFi,
            Self::Bluetooth => WinRadioKind::Bluetooth,
        }
    }
}

fn find_radio(kind: RadioKind) -> Option<Radio> {
    Radio::GetRadiosAsync()
        .ok()?
        .get()
        .ok()?
        .into_iter()
        .find(|r| r.Kind().ok() == Some(kind.windows()))
}

/// Whether the radio is on, or `None` if the machine has none.
pub fn radio(kind: RadioKind) -> Option<bool> {
    let _com = Com::init();
    find_radio(kind)?
        .State()
        .ok()
        .map(|state| state == RadioState::On)
}

/// Turns the radio on or off.
pub fn set_radio(kind: RadioKind, on: bool) -> bool {
    let _com = Com::init();
    let Some(radio) = find_radio(kind) else {
        return false;
    };
    let allowed = Radio::RequestAccessAsync()
        .ok()
        .and_then(|op| op.get().ok())
        .is_some_and(|status| status == RadioAccessStatus::Allowed);
    let state = if on { RadioState::On } else { RadioState::Off };
    allowed
        && radio
            .SetStateAsync(state)
            .ok()
            .and_then(|op| op.get().ok())
            .is_some_and(|status| status == RadioAccessStatus::Allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The machine decides what exists, so this only checks the calls run and agree.
    #[test]
    fn reads_the_system_without_failing() {
        let volume = volume();
        if let Some(v) = volume {
            assert!(v.percent <= 100);
        }
        if let Some(b) = battery() {
            assert!(b.percent <= 100);
        }
        let _ = network();
        let _ = radio(RadioKind::Wifi);
        let _ = radio(RadioKind::Bluetooth);
        println!("volume {volume:?}, network {:?}", network());
    }
}
