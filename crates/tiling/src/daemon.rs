use std::{
    collections::{HashMap, HashSet},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail};
use tiling_config::Loaded;
use tiling_core::{
    Command, Effect, LayoutKind, ManageOptions, MonitorInfo, Rect, WindowId, animation::lerp_rect,
};
use tiling_ipc::{Event, Incoming, Query, Request, Response, Server};
use tiling_platform::{
    Discovered, Hotkeys, PlaceError, Tray, TrayEvent, WinEvent, WinEventHooks, Windows, cursor_pos,
    desktops::{self as vd, DesktopChange, DesktopId},
    foreground, is_shell_window, monitors,
};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::{Instant, MissedTickBehavior, interval, sleep_until},
};

use crate::{
    desktops::Desktops,
    paths::{self, Restore},
};

/// How long an event we caused ourselves (hide/show) may take to arrive.
const SELF_EVENT_WINDOW: Duration = Duration::from_secs(2);
const RELOAD_DEBOUNCE: Duration = Duration::from_millis(200);

struct Tween {
    from: Rect,
    to: Rect,
    current: Rect,
    start: Instant,
}

#[derive(Default, PartialEq)]
struct Published {
    workspace: Option<u8>,
    window: Option<(isize, String, String)>,
    layout: Option<LayoutKind>,
}

struct Receivers {
    events: UnboundedReceiver<WinEvent>,
    desktops: UnboundedReceiver<DesktopChange>,
    hotkeys: UnboundedReceiver<usize>,
    tray: UnboundedReceiver<TrayEvent>,
    ipc: UnboundedReceiver<Incoming>,
    config: UnboundedReceiver<()>,
}

struct Daemon {
    desktops: Desktops,
    windows: Windows,
    loaded: Loaded,
    config_path: PathBuf,
    commands: Vec<Command>,
    hotkeys: Hotkeys,
    tray: Tray,
    paused: bool,
    quit: bool,
    monitors: Vec<MonitorInfo>,
    /// The last rect we placed each window at (or are animating it towards).
    placed: HashMap<WindowId, Rect>,
    tweens: HashMap<WindowId, Tween>,
    /// Windows the user is dragging / resizing with the mouse.
    moving: HashSet<WindowId>,
    pending_show: HashMap<WindowId, (u32, Instant)>,
    pending_hide: HashMap<WindowId, (u32, Instant)>,
    /// Windows excluded by rules or that we can't control (elevated).
    ignored: HashSet<WindowId>,
    bordered: Option<WindowId>,
    /// Windows handled as undecorated; `true` if we actually removed a title bar.
    undecorated: HashMap<WindowId, bool>,
    /// Per-window `decorations` overrides from rules.
    decoration_rules: HashMap<WindowId, bool>,
    subscribers: Vec<UnboundedSender<Response>>,
    published: Published,
    saved_restore: Restore,
}

fn bump(map: &mut HashMap<WindowId, (u32, Instant)>, id: WindowId) {
    let entry = map.entry(id).or_insert((0, Instant::now()));
    entry.0 += 1;
    entry.1 = Instant::now();
}

/// Consumes one expected self-inflicted event, if there is a recent one.
fn consume(map: &mut HashMap<WindowId, (u32, Instant)>, id: WindowId) -> bool {
    match map.get_mut(&id) {
        Some((count, at)) if *count > 0 && at.elapsed() < SELF_EVENT_WINDOW => {
            *count -= 1;
            if *count == 0 {
                map.remove(&id);
            }
            true
        }
        Some(_) => {
            map.remove(&id);
            false
        }
        None => false,
    }
}

/// Runs a command line through `start`, detached and without a console window.
fn spawn(command_line: &str) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let result = std::process::Command::new("cmd")
        .raw_arg(format!("/C start \"\" {command_line}"))
        .creation_flags(CREATE_NO_WINDOW)
        .spawn();
    if let Err(e) = result {
        tracing::error!("failed to run `{command_line}`: {e}");
    }
}

fn open(path: &Path) {
    spawn(&format!("\"{}\"", path.display()));
}

/// Starts `tiling --watchdog <pid>`: a detached process that undoes our window changes
/// when this process exits for any reason, including being killed.
fn spawn_watchdog() {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            tracing::warn!("no watchdog: {e}");
            return;
        }
    };
    let pid = std::process::id().to_string();
    let launch = |flags: u32| {
        std::process::Command::new(&exe)
            .args(["--watchdog", &pid])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(flags)
            .spawn()
    };
    let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
    // Escape our job object if allowed, so killing the job doesn't take the watchdog too.
    match launch(base | CREATE_BREAKAWAY_FROM_JOB).or_else(|_| launch(base)) {
        Ok(child) => tracing::debug!("watchdog started (pid {})", child.id()),
        Err(e) => tracing::warn!("failed to start watchdog: {e}"),
    }
}

/// Whether a window currently lives on a native desktop other than the visible one.
fn on_other_desktop(raw: isize) -> bool {
    matches!((vd::desktop_of(raw), vd::current_desktop()), (Some(d), Some(c)) if d != c)
}

pub async fn run(config_path: PathBuf, loaded: Loaded) -> anyhow::Result<()> {
    if tiling_ipc::is_running().await {
        bail!("tiling is already running");
    }
    let server = Server::bind().context("failed to create the IPC pipe")?;
    let _com = vd::init_com();

    // Recover windows a crashed instance left hidden or undecorated.
    paths::load_restore().apply();
    paths::save_restore(&Restore::default());
    spawn_watchdog();

    let (hotkey_tx, hotkeys_rx) = mpsc::unbounded_channel();
    let (tray_tx, tray_rx) = mpsc::unbounded_channel();
    let (ipc_tx, ipc_rx) = mpsc::unbounded_channel();
    let (config_tx, config_rx) = mpsc::unbounded_channel();
    let (desktop_tx, desktop_rx) = mpsc::unbounded_channel();

    let (hooks, events) = WinEventHooks::install().await.context("failed to hook window events")?;
    let _desktop_listener = vd::listen(desktop_tx);
    let hotkeys = Hotkeys::start(hotkey_tx);
    let tray = Tray::spawn(tray_tx);
    let _watcher = tiling_config::watch(&config_path, move || {
        let _ = config_tx.send(());
    })
    .inspect_err(|e| tracing::warn!("config hot reload unavailable: {e}"))
    .ok();
    tokio::spawn(server.serve(ipc_tx));

    let settings = loaded.config.layout_settings();
    let layout = loaded.config.general.layout;
    let current = vd::current_desktop().unwrap_or_default();
    let mut daemon = Daemon {
        desktops: Desktops::new(current, settings, layout),
        windows: Windows::new(),
        loaded,
        config_path,
        commands: Vec::new(),
        hotkeys,
        tray,
        paused: false,
        quit: false,
        monitors: Vec::new(),
        placed: HashMap::new(),
        tweens: HashMap::new(),
        moving: HashSet::new(),
        pending_show: HashMap::new(),
        pending_hide: HashMap::new(),
        ignored: HashSet::new(),
        bordered: None,
        undecorated: HashMap::new(),
        decoration_rules: HashMap::new(),
        subscribers: Vec::new(),
        published: Published::default(),
        saved_restore: Restore::default(),
    };
    daemon.install_binds();
    daemon.startup();
    tracing::info!("tiling started");

    daemon
        .event_loop(Receivers {
            events,
            desktops: desktop_rx,
            hotkeys: hotkeys_rx,
            tray: tray_rx,
            ipc: ipc_rx,
            config: config_rx,
        })
        .await;

    daemon.shutdown();
    hooks.unhook().await;
    tracing::info!("tiling stopped");
    Ok(())
}

impl Daemon {
    async fn event_loop(&mut self, mut rx: Receivers) {
        let mut frame = interval(Duration::from_millis(16));
        frame.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut sweep = interval(Duration::from_secs(2));
        sweep.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut reload_at: Option<Instant> = None;

        while !self.quit {
            let reload = async {
                match reload_at {
                    Some(at) => sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                Some(event) = rx.events.recv() => self.on_event(event),
                Some(change) = rx.desktops.recv() => self.on_desktop(change),
                Some(index) = rx.hotkeys.recv() => {
                    if let Some(command) = self.commands.get(index).cloned() {
                        tracing::debug!("hotkey: {command}");
                        self.run_command(command);
                    }
                }
                Some(event) = rx.tray.recv() => self.on_tray(event),
                Some(incoming) = rx.ipc.recv() => self.on_ipc(incoming),
                Some(()) = rx.config.recv() => reload_at = Some(Instant::now() + RELOAD_DEBOUNCE),
                () = reload => {
                    reload_at = None;
                    self.reload();
                }
                _ = frame.tick(), if !self.tweens.is_empty() => self.animate(),
                _ = sweep.tick() => self.sweep(),
                _ = tokio::signal::ctrl_c() => self.quit = true,
            }
        }
    }

    fn startup(&mut self) {
        self.refresh_monitors();
        self.discover_existing();
        if let Some(raw) = foreground() {
            self.on_foreground(raw);
        }
    }

    /// Manages every window that is visible right now, without stealing focus.
    fn discover_existing(&mut self) {
        for found in self.windows.discover_all() {
            if vd::is_pinned(found.id.0) {
                self.windows.forget(found.id);
                continue;
            }
            let effects = self.manage(found, false);
            self.apply(effects);
        }
    }

    fn shutdown(&mut self) {
        for id in self.desktops.hidden() {
            self.windows.show(id);
        }
        for id in self.desktops.managed() {
            self.windows.set_border(id, None);
        }
        for (id, changed) in std::mem::take(&mut self.undecorated) {
            if changed {
                self.windows.set_decorated(id, true);
            }
        }
        paths::save_restore(&Restore::default());
        self.tray.shutdown();
    }

    fn install_binds(&mut self) {
        let binds: Vec<_> = self
            .loaded
            .binds
            .iter()
            .map(|(bind, command)| (bind.clone(), *command == Command::TogglePause))
            .collect();
        self.commands = self.loaded.binds.iter().map(|(_, c)| c.clone()).collect();
        for error in self.hotkeys.set_binds(&binds) {
            tracing::error!("{error}");
        }
    }

    fn reload(&mut self) {
        match Loaded::load(&self.config_path) {
            Ok(loaded) => {
                self.loaded = loaded;
                self.ignored.clear();
                self.install_binds();
                self.decoration_rules = self
                    .desktops
                    .managed()
                    .into_iter()
                    .filter_map(|id| {
                        let info = self.windows.info(id)?;
                        self.loaded.rules.resolve(&info).decorations.map(|d| (id, d))
                    })
                    .collect();
                let config = &self.loaded.config;
                let effects = self.desktops.set_settings(config.layout_settings(), config.general.layout);
                self.repaint_borders();
                self.apply(effects);
                tracing::info!("config reloaded");
            }
            Err(e) => tracing::error!("keeping previous config: {e}"),
        }
    }

    fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.hotkeys.set_paused(paused);
        self.tray.set_paused(paused);
        tracing::info!("tiling {}", if paused { "paused" } else { "resumed" });
        if !paused {
            self.refresh_monitors();
            self.sweep();
            self.discover_existing();
            let effects = self.desktops.current().sync();
            self.apply(effects);
        }
    }

    fn focused(&self) -> Option<WindowId> {
        self.desktops.current_ref().focused()
    }

    fn run_command(&mut self, command: Command) {
        match command {
            Command::Reload => self.reload(),
            Command::TogglePause => self.set_paused(!self.paused),
            Command::Exit => self.quit = true,
            Command::Exec(line) => spawn(&line),
            Command::Desktop(target) => vd::switch_desktop(target),
            _ if self.paused => {}
            Command::MoveToDesktop(target) => {
                if let Some(id) = self.focused() {
                    vd::move_to_desktop(id.0, target);
                    self.check_desktop(id);
                }
            }
            Command::Pin => {
                if let Some(id) = self.focused() {
                    vd::toggle_pin(id.0);
                    self.check_desktop(id);
                }
            }
            command => {
                let effects = self.desktops.current().apply(&command);
                self.apply(effects);
            }
        }
    }

    fn refresh_monitors(&mut self) {
        let current = monitors();
        if current.is_empty() || current == self.monitors {
            return;
        }
        tracing::info!("monitors: {current:?}");
        self.monitors.clone_from(&current);
        let effects = self.desktops.set_monitors(current);
        self.apply(effects);
    }

    /// Starts managing a discovered window on the native desktop it lives on. `opened` is
    /// false for windows that already existed (startup / resume), which must not steal focus.
    fn manage(&mut self, found: Discovered, opened: bool) -> Vec<Effect> {
        let outcome = self.loaded.rules.resolve(&found.info);
        if outcome.ignore {
            self.windows.forget(found.id);
            self.ignored.insert(found.id);
            return Vec::new();
        }
        tracing::debug!("managing {:?} {:?}", found.id, found.info);
        if let Some(decorations) = outcome.decorations {
            self.decoration_rules.insert(found.id, decorations);
        }
        if self.loaded.config.general.borders {
            self.windows.set_border(found.id, Some(self.loaded.config.general.border_inactive));
        }
        let desktop = vd::desktop_of(found.id.0).unwrap_or_else(|| self.desktops.current_id());
        let options = ManageOptions {
            floating: outcome.float.unwrap_or(found.floating_hint),
            workspace: outcome.workspace,
            monitor: outcome.monitor,
            rect: found.rect,
            focus: opened && self.loaded.config.general.focus_new_windows,
        };
        self.desktops.open(desktop, found.id, found.info, options)
    }

    fn try_manage(&mut self, raw: isize) {
        if self.ignored.contains(&WindowId(raw)) {
            return;
        }
        if let Some(found) = self.windows.discover(raw) {
            // Windows shown on every desktop float above the tiling.
            if vd::is_pinned(raw) {
                self.windows.forget(found.id);
                return;
            }
            let effects = self.manage(found, true);
            self.apply(effects);
        }
    }

    fn unmanage(&mut self, id: WindowId) {
        tracing::debug!("unmanaging {id:?}");
        let alive = self.windows.is_alive(id);
        if self.undecorated.remove(&id) == Some(true) && alive {
            self.windows.set_decorated(id, true);
        }
        if self.desktops.is_hidden(id) && alive {
            self.windows.show(id);
        }
        self.decoration_rules.remove(&id);
        let effects = self.desktops.close(id);
        self.windows.forget(id);
        self.placed.remove(&id);
        self.tweens.remove(&id);
        self.moving.remove(&id);
        self.pending_hide.remove(&id);
        self.pending_show.remove(&id);
        if self.bordered == Some(id) {
            self.bordered = None;
        }
        self.apply(effects);
    }

    /// Re-homes a window after it was moved to another native desktop or (un)pinned.
    fn check_desktop(&mut self, id: WindowId) {
        if vd::is_pinned(id.0) {
            if self.desktops.is_managed(id) {
                self.unmanage(id);
            }
            return;
        }
        let Some(desktop) = vd::desktop_of(id.0) else { return };
        match self.desktops.owner(id) {
            None => self.try_manage(id.0),
            Some(owner) if owner != desktop => self.migrate(id, desktop),
            Some(_) => {}
        }
    }

    fn migrate(&mut self, id: WindowId, desktop: DesktopId) {
        tracing::debug!("{id:?} moved to desktop {desktop:032x}");
        let floating = self.desktops.is_floating(id);
        if self.desktops.is_hidden(id) {
            bump(&mut self.pending_show, id);
            self.windows.show(id);
        }
        let effects = self.desktops.close(id);
        self.apply(effects);
        self.placed.remove(&id);
        self.tweens.remove(&id);
        let info = self.windows.info(id).unwrap_or_default();
        let rect = self.windows.frame(id).unwrap_or_default();
        let options = ManageOptions { floating, rect, ..ManageOptions::default() };
        let effects = self.desktops.open(desktop, id, info, options);
        self.apply(effects);
    }

    fn switch_desktop(&mut self, desktop: DesktopId) {
        if desktop == self.desktops.current_id() {
            return;
        }
        tracing::debug!("switched to desktop {desktop:032x}");
        let effects = self.desktops.switch_to(desktop);
        self.apply(effects);
        // Windows on this desktop we haven't seen yet (e.g. created while it was hidden).
        self.discover_existing();
        if let Some(raw) = foreground() {
            self.on_foreground(raw);
        }
    }

    fn on_desktop(&mut self, change: DesktopChange) {
        if self.paused {
            return;
        }
        match change {
            DesktopChange::Switched(desktop) => self.switch_desktop(desktop),
            DesktopChange::Window(raw) => self.check_desktop(WindowId(raw)),
            DesktopChange::Changed => {
                // A removed desktop's windows move to its neighbour.
                for id in self.desktops.managed() {
                    self.check_desktop(id);
                }
            }
        }
    }

    fn on_foreground(&mut self, raw: isize) {
        let id = WindowId(raw);
        if !self.desktops.is_managed(id) {
            self.try_manage(raw);
            if !self.desktops.is_managed(id) {
                if !is_shell_window(raw) {
                    let effects = self.desktops.current().window_focused(id);
                    self.apply(effects);
                }
                return;
            }
        }
        // Focus can arrive before the desktop switch notification.
        if self.desktops.owner(id) != Some(self.desktops.current_id())
            && let Some(live) = vd::current_desktop()
        {
            self.switch_desktop(live);
        }
        let effects = self.desktops.with_owner(id, |s| s.window_focused(id));
        self.apply(effects);
    }

    fn on_event(&mut self, event: WinEvent) {
        if self.paused {
            return;
        }
        match event {
            WinEvent::Show(raw) | WinEvent::Uncloaked(raw) => {
                let id = WindowId(raw);
                if !self.desktops.is_managed(id) {
                    self.try_manage(raw);
                } else if !consume(&mut self.pending_show, id)
                    && self.desktops.is_hidden(id)
                    && self.desktops.owner(id) == Some(self.desktops.current_id())
                {
                    // The app re-showed a window we hid: follow it to its workspace.
                    let effects = self.desktops.with_owner(id, |s| s.window_focused(id));
                    self.apply(effects);
                }
            }
            WinEvent::Hide(raw) | WinEvent::Cloaked(raw) => {
                let id = WindowId(raw);
                if self.desktops.is_managed(id)
                    && !consume(&mut self.pending_hide, id)
                    && !self.windows.is_visible(id)
                {
                    // The shell cloaks windows on other desktops: keep managing those.
                    if on_other_desktop(raw) {
                        self.check_desktop(id);
                    } else {
                        self.unmanage(id);
                    }
                }
            }
            WinEvent::Destroy(raw) => {
                let id = WindowId(raw);
                self.ignored.remove(&id);
                if self.desktops.is_managed(id) {
                    self.unmanage(id);
                }
            }
            WinEvent::Foreground(raw) => self.on_foreground(raw),
            WinEvent::MinimizeStart(raw) => {
                let id = WindowId(raw);
                let effects = self.desktops.with_owner(id, |s| s.window_minimized(id));
                self.apply(effects);
            }
            WinEvent::MinimizeEnd(raw) => {
                let id = WindowId(raw);
                if self.desktops.is_managed(id) {
                    let effects = self.desktops.with_owner(id, |s| s.window_restored(id));
                    self.apply(effects);
                } else {
                    self.try_manage(raw);
                }
            }
            WinEvent::MoveSizeStart(raw) => {
                let id = WindowId(raw);
                if self.desktops.is_managed(id) {
                    self.moving.insert(id);
                    self.tweens.remove(&id);
                }
            }
            WinEvent::MoveSizeEnd(raw) => {
                let id = WindowId(raw);
                if self.moving.remove(&id) {
                    self.placed.remove(&id);
                    if let Some(rect) = self.windows.frame(id) {
                        let cursor = cursor_pos();
                        let effects = self.desktops.with_owner(id, |s| s.window_moved(id, rect, cursor));
                        self.apply(effects);
                    }
                }
            }
            WinEvent::NameChange(raw) => {
                let id = WindowId(raw);
                if self.desktops.is_managed(id) {
                    if self.focused() == Some(id)
                        && let Some(info) = self.windows.info(id)
                    {
                        self.desktops.current().set_info(id, info);
                        self.publish();
                    }
                } else if foreground() == Some(raw) {
                    // Some apps only set a title after showing their window.
                    self.try_manage(raw);
                }
            }
            WinEvent::LocationChange(_) => {}
        }
    }

    fn on_tray(&mut self, event: TrayEvent) {
        match event {
            TrayEvent::TogglePause => self.set_paused(!self.paused),
            TrayEvent::Reload => self.reload(),
            TrayEvent::OpenConfig => open(&self.config_path),
            TrayEvent::OpenLogs => open(&paths::log_dir()),
            TrayEvent::Quit => self.quit = true,
            TrayEvent::DisplayChanged => self.refresh_monitors(),
        }
    }

    fn on_ipc(&mut self, incoming: Incoming) {
        let Incoming { request, reply } = incoming;
        let response = match request {
            Request::Dispatch { command } => match command.parse::<Command>() {
                Ok(command) => {
                    self.run_command(command);
                    Response::Ok
                }
                Err(e) => Response::Error { message: e.to_string() },
            },
            Request::Query { what } => {
                let state = self.desktops.current_ref();
                match what {
                    Query::Monitors => Response::Monitors { monitors: state.snapshot_monitors() },
                    Query::Workspaces => Response::Workspaces { workspaces: state.snapshot_workspaces() },
                    Query::Windows => Response::Windows { windows: state.snapshot_windows() },
                    Query::ActiveWindow => Response::ActiveWindow { window: state.snapshot_active_window() },
                }
            }
            Request::Subscribe => {
                for event in self.current_events() {
                    let _ = reply.send(Response::Event { event });
                }
                self.subscribers.push(reply);
                return;
            }
        };
        let _ = reply.send(response);
    }

    /// Applies effects from the core state to the real windows.
    fn apply(&mut self, effects: Vec<Effect>) {
        let shown: HashSet<WindowId> = effects
            .iter()
            .filter_map(|e| if let Effect::Show(id) = e { Some(*id) } else { None })
            .collect();
        let mut failed = Vec::new();
        for effect in effects {
            match effect {
                Effect::Place { id, rect } => {
                    if let Err(id) = self.place(id, rect, !shown.contains(&id)) {
                        failed.push(id);
                    }
                }
                Effect::Show(id) => {
                    bump(&mut self.pending_show, id);
                    self.windows.show(id);
                }
                Effect::Hide(id) => {
                    bump(&mut self.pending_hide, id);
                    self.tweens.remove(&id);
                    self.windows.hide(id);
                }
                Effect::Focus(id) => self.windows.focus(id),
                Effect::Close(id) => self.windows.close(id),
                Effect::Exec(line) => spawn(&line),
            }
        }
        for id in failed {
            self.ignored.insert(id);
            self.unmanage(id);
        }
        self.update_decorations();
        self.update_borders();
        self.persist_restore();
        self.publish();
    }

    fn wants_decorations(&self, id: WindowId) -> bool {
        self.decoration_rules
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.loaded.config.general.decorations || self.desktops.is_floating(id))
    }

    /// Adds or removes title bars so every managed window matches the config.
    fn update_decorations(&mut self) {
        for id in self.desktops.managed() {
            let want = self.wants_decorations(id);
            match (want, self.undecorated.get(&id).copied()) {
                (false, None) => {
                    let changed = self.windows.set_decorated(id, false);
                    self.undecorated.insert(id, changed);
                }
                (true, Some(changed)) => {
                    if changed {
                        self.windows.set_decorated(id, true);
                    }
                    self.undecorated.remove(&id);
                }
                _ => {}
            }
        }
    }

    fn place(&mut self, id: WindowId, rect: Rect, may_animate: bool) -> Result<(), WindowId> {
        if self.moving.contains(&id) {
            return Ok(());
        }
        let previous = self.placed.insert(id, rect);
        if previous == Some(rect) {
            return Ok(());
        }
        let animations = &self.loaded.config.animations;
        if animations.enabled
            && may_animate
            && !self.desktops.is_hidden(id)
            && self.desktops.owner(id) == Some(self.desktops.current_id())
            && let Some(previous) = previous
        {
            let from = self.tweens.get(&id).map_or(previous, |t| t.current);
            self.tweens.insert(id, Tween { from, to: rect, current: from, start: Instant::now() });
            return Ok(());
        }
        self.tweens.remove(&id);
        self.place_now(id, rect)
    }

    fn place_now(&mut self, id: WindowId, rect: Rect) -> Result<(), WindowId> {
        match self.windows.place(id, rect) {
            Ok(()) => Ok(()),
            Err(PlaceError::Gone) => Err(id),
            Err(PlaceError::AccessDenied) => {
                tracing::warn!("cannot control {id:?} (elevated?), ignoring it");
                Err(id)
            }
            Err(PlaceError::Other(e)) => {
                tracing::debug!("placing {id:?} failed: {e}");
                Ok(())
            }
        }
    }

    fn animate(&mut self) {
        let config = &self.loaded.config.animations;
        let duration = Duration::from_millis(config.duration_ms.max(1)).as_secs_f32();
        let curve = config.curve();
        let mut frames = Vec::new();
        self.tweens.retain(|id, tween| {
            let progress = tween.start.elapsed().as_secs_f32() / duration;
            if progress >= 1.0 {
                frames.push((*id, tween.to));
                false
            } else {
                tween.current = lerp_rect(tween.from, tween.to, curve.ease(progress));
                frames.push((*id, tween.current));
                true
            }
        });
        let failed: Vec<_> = frames
            .into_iter()
            .filter_map(|(id, rect)| self.place_now(id, rect).err())
            .collect();
        for id in failed {
            self.unmanage(id);
        }
    }

    /// Drops windows that disappeared without us noticing.
    fn sweep(&mut self) {
        let dead: Vec<_> = self
            .desktops
            .managed()
            .into_iter()
            .filter(|id| !self.windows.is_alive(*id))
            .collect();
        for id in dead {
            self.unmanage(id);
        }
    }

    fn update_borders(&mut self) {
        let general = &self.loaded.config.general;
        if !general.borders {
            return;
        }
        let focused = self.desktops.current_ref().focused();
        if focused == self.bordered {
            return;
        }
        if let Some(old) = self.bordered {
            self.windows.set_border(old, Some(general.border_inactive));
        }
        if let Some(new) = focused {
            self.windows.set_border(new, Some(general.border_active));
        }
        self.bordered = focused;
    }

    fn repaint_borders(&mut self) {
        let general = &self.loaded.config.general;
        let color = general.borders.then_some(general.border_inactive);
        for id in self.desktops.managed() {
            self.windows.set_border(id, color);
        }
        self.bordered = None;
    }

    /// Records what must be undone if we die, for the panic hook and the watchdog.
    fn persist_restore(&mut self) {
        let mut restore = Restore {
            hidden: self.desktops.hidden().into_iter().map(|id| id.0).collect(),
            undecorated: self.undecorated.iter().filter(|(_, changed)| **changed).map(|(id, _)| id.0).collect(),
        };
        restore.hidden.sort_unstable();
        restore.undecorated.sort_unstable();
        if restore != self.saved_restore {
            paths::save_restore(&restore);
            self.saved_restore = restore;
        }
    }

    fn snapshot(&self) -> Published {
        let state = self.desktops.current_ref();
        Published {
            workspace: state.focused_workspace(),
            window: state.snapshot_active_window().map(|w| (w.id, w.title, w.exe)),
            layout: state.active_layout(),
        }
    }

    fn current_events(&self) -> Vec<Event> {
        let now = self.snapshot();
        let mut events = Vec::new();
        events.extend(now.workspace.map(|id| Event::Workspace { id }));
        events.push(match now.window {
            Some((id, title, exe)) => Event::ActiveWindow { id: Some(id), title: Some(title), exe: Some(exe) },
            None => Event::ActiveWindow { id: None, title: None, exe: None },
        });
        events.extend(now.layout.map(|layout| Event::Layout { layout }));
        events
    }

    /// Notifies IPC subscribers about changes to the workspace, active window and layout.
    fn publish(&mut self) {
        let now = self.snapshot();
        if now == self.published {
            return;
        }
        let mut events = Vec::new();
        if now.workspace != self.published.workspace
            && let Some(id) = now.workspace
        {
            events.push(Event::Workspace { id });
        }
        if now.window != self.published.window {
            events.push(match &now.window {
                Some((id, title, exe)) => Event::ActiveWindow {
                    id: Some(*id),
                    title: Some(title.clone()),
                    exe: Some(exe.clone()),
                },
                None => Event::ActiveWindow { id: None, title: None, exe: None },
            });
        }
        if now.layout != self.published.layout
            && let Some(layout) = now.layout
        {
            events.push(Event::Layout { layout });
        }
        self.published = now;
        self.subscribers.retain(|tx| {
            events.iter().all(|event| tx.send(Response::Event { event: event.clone() }).is_ok())
        });
    }
}
