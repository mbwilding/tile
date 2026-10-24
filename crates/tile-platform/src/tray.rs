use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

use tokio::sync::mpsc;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};
use winsafe::{self as w, co, gui, prelude::*};

/// Something that happened on the tray / message thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    /// Pause or resume the window manager.
    ToggleTile,
    /// Pause or resume the key remapper.
    ToggleKey,
    ToggleLogging,
    Reload,
    OpenConfig,
    OpenLogs,
    Quit,
    /// Monitors or the work area changed.
    DisplayChanged,
    /// The session was locked, unlocked or switched.
    SessionChanged,
}

const POLL_TIMER: usize = 1;

/// The tray icon, living on a dedicated thread with a hidden window and message loop.
pub struct Tray {
    shutdown: Arc<AtomicBool>,
    checks: Arc<Checks>,
    thread: Option<thread::JoinHandle<()>>,
}

/// What the menu's check marks should show, set by the daemon.
#[derive(Default)]
struct Checks {
    tile_paused: AtomicBool,
    key_paused: AtomicBool,
    logging: AtomicBool,
}

/// A 32x32 icon: four rounded tiles in the accent color.
fn icon() -> Option<Icon> {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let in_tile = |v: u32| (2..15).contains(&v) || (17..30).contains(&v);
            let corner = |v: u32| matches!(v, 2 | 14 | 17 | 29);
            let filled = in_tile(x) && in_tile(y) && !(corner(x) && corner(y));
            let primary = x < 16 && y < 16;
            let pixel = match (filled, primary) {
                (false, _) => [0, 0, 0, 0],
                (true, true) => [0x89, 0xb4, 0xfa, 0xff],
                (true, false) => [0xcd, 0xd6, 0xf4, 0xff],
            };
            rgba.extend_from_slice(&pixel);
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).ok()
}

struct Items {
    /// Checked while the window manager runs (not paused).
    tile: CheckMenuItem,
    /// Checked while the key remapper runs (not paused).
    key: CheckMenuItem,
    logging: CheckMenuItem,
    reload: MenuId,
    open_config: MenuId,
    open_logs: MenuId,
    quit: MenuId,
}

impl Items {
    fn sync(&self, checks: &Checks) {
        let set = |item: &CheckMenuItem, want: bool| {
            if item.is_checked() != want {
                item.set_checked(want);
            }
        };
        set(&self.tile, !checks.tile_paused.load(Ordering::Relaxed));
        set(&self.key, !checks.key_paused.load(Ordering::Relaxed));
        set(&self.logging, checks.logging.load(Ordering::Relaxed));
    }
}

fn build(checks: &Checks) -> Result<(TrayIcon, Items), String> {
    let tile = CheckMenuItem::new("Tile", true, true, None);
    let key = CheckMenuItem::new("Key", true, true, None);
    let logging = CheckMenuItem::new("Logging", true, false, None);
    let reload = MenuItem::new("Reload config", true, None);
    let open_config = MenuItem::new("Open config", true, None);
    let open_logs = MenuItem::new("Open logs", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &tile,
        &key,
        &PredefinedMenuItem::separator(),
        &reload,
        &open_config,
        &PredefinedMenuItem::separator(),
        &logging,
        &open_logs,
        &PredefinedMenuItem::separator(),
        &quit,
    ])
    .map_err(|e| e.to_string())?;
    let mut builder = TrayIconBuilder::new()
        .with_tooltip("Tile")
        .with_menu(Box::new(menu));
    if let Some(icon) = icon() {
        builder = builder.with_icon(icon);
    }
    let tray = builder.build().map_err(|e| e.to_string())?;
    let items = Items {
        reload: reload.id().clone(),
        open_config: open_config.id().clone(),
        open_logs: open_logs.id().clone(),
        quit: quit.id().clone(),
        tile,
        key,
        logging,
    };
    items.sync(checks);
    Ok((tray, items))
}

impl Tray {
    pub fn spawn(tx: mpsc::UnboundedSender<TrayEvent>) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let checks = Arc::new(Checks::default());
        let (stop, state) = (shutdown.clone(), checks.clone());
        let thread = thread::Builder::new()
            .name("tray".into())
            .spawn(move || run(&tx, &stop, &state))
            .expect("spawn tray thread");
        Self {
            shutdown,
            checks,
            thread: Some(thread),
        }
    }

    /// Reflects the window manager's pause state in the menu.
    pub fn set_tile_paused(&self, paused: bool) {
        self.checks.tile_paused.store(paused, Ordering::Relaxed);
    }

    /// Reflects the key remapper's pause state in the menu.
    pub fn set_key_paused(&self, paused: bool) {
        self.checks.key_paused.store(paused, Ordering::Relaxed);
    }

    pub fn set_logging(&self, logging: bool) {
        self.checks.logging.store(logging, Ordering::Relaxed);
    }

    /// Removes the icon and stops the tray thread.
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(tx: &mpsc::UnboundedSender<TrayEvent>, shutdown: &Arc<AtomicBool>, checks: &Arc<Checks>) {
    let wnd = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Tile",
        class_name: "tile-message-window",
        ..Default::default()
    });
    let state: Rc<RefCell<Option<(TrayIcon, Items)>>> = Rc::new(RefCell::new(None));

    wnd.on().wm_create({
        let (wnd, state, checks) = (wnd.clone(), state.clone(), checks.clone());
        move |_| {
            match build(&checks) {
                Ok(built) => *state.borrow_mut() = Some(built),
                Err(e) => tracing::error!("failed to create tray icon: {e}"),
            }
            if !tile_hook::watch_sessions(wnd.hwnd().ptr() as isize) {
                tracing::warn!(
                    "can't watch for the session locking; remapped keys may stick after unlocking"
                );
            }
            wnd.hwnd().SetTimer(POLL_TIMER, 100, None)?;
            Ok(0)
        }
    });

    wnd.on().wm_timer(POLL_TIMER, {
        let (wnd, state, tx, shutdown, checks) = (
            wnd.clone(),
            state.clone(),
            tx.clone(),
            shutdown.clone(),
            checks.clone(),
        );
        move || {
            if shutdown.load(Ordering::Relaxed) {
                state.borrow_mut().take();
                tile_hook::unwatch_sessions(wnd.hwnd().ptr() as isize);
                w::PostQuitMessage(0);
                return Ok(());
            }
            let state = state.borrow();
            let Some((_, items)) = state.as_ref() else {
                return Ok(());
            };
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                let mapped = if event.id == *items.tile.id() {
                    Some(TrayEvent::ToggleTile)
                } else if event.id == *items.key.id() {
                    Some(TrayEvent::ToggleKey)
                } else if event.id == *items.logging.id() {
                    Some(TrayEvent::ToggleLogging)
                } else if event.id == items.reload {
                    Some(TrayEvent::Reload)
                } else if event.id == items.open_config {
                    Some(TrayEvent::OpenConfig)
                } else if event.id == items.open_logs {
                    Some(TrayEvent::OpenLogs)
                } else if event.id == items.quit {
                    Some(TrayEvent::Quit)
                } else {
                    None
                };
                if let Some(e) = mapped {
                    let _ = tx.send(e);
                }
            }
            items.sync(&checks);
            Ok(())
        }
    });

    wnd.on().wm_display_change({
        let tx = tx.clone();
        move |_| {
            let _ = tx.send(TrayEvent::DisplayChanged);
            Ok(())
        }
    });

    wnd.on().wm_wts_session_change({
        let tx = tx.clone();
        move |_| {
            let _ = tx.send(TrayEvent::SessionChanged);
            Ok(())
        }
    });

    // WM_SETTINGCHANGE (same value as WM_WININICHANGE) fires when the work area changes.
    wnd.on().wm(co::WM::WININICHANGE, {
        let tx = tx.clone();
        move |_| {
            let _ = tx.send(TrayEvent::DisplayChanged);
            Ok(0)
        }
    });

    if let Err(e) = wnd.run_main(Some(co::SW::HIDE)) {
        tracing::error!("tray message loop failed: {e}");
    }
}
