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
    TogglePause,
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
    paused: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
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
    pause: CheckMenuItem,
    reload: MenuId,
    open_config: MenuId,
    open_logs: MenuId,
    quit: MenuId,
}

fn build(paused: bool) -> Result<(TrayIcon, Items), String> {
    let pause = CheckMenuItem::new("Paused", true, paused, None);
    let reload = MenuItem::new("Reload config", true, None);
    let open_config = MenuItem::new("Open config", true, None);
    let open_logs = MenuItem::new("Open logs", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &pause,
        &PredefinedMenuItem::separator(),
        &reload,
        &open_config,
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
        pause,
    };
    Ok((tray, items))
}

impl Tray {
    pub fn spawn(tx: mpsc::UnboundedSender<TrayEvent>) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let paused = Arc::new(AtomicBool::new(false));
        let (stop, pause_state) = (shutdown.clone(), paused.clone());
        let thread = thread::Builder::new()
            .name("tray".into())
            .spawn(move || run(&tx, &stop, &pause_state))
            .expect("spawn tray thread");
        Self {
            shutdown,
            paused,
            thread: Some(thread),
        }
    }

    /// Reflects the pause state in the menu check mark.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// Removes the icon and stops the tray thread.
    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(
    tx: &mpsc::UnboundedSender<TrayEvent>,
    shutdown: &Arc<AtomicBool>,
    paused: &Arc<AtomicBool>,
) {
    let wnd = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Tile",
        class_name: "tile-message-window",
        ..Default::default()
    });
    let state: Rc<RefCell<Option<(TrayIcon, Items)>>> = Rc::new(RefCell::new(None));

    wnd.on().wm_create({
        let (wnd, state, paused) = (wnd.clone(), state.clone(), paused.clone());
        move |_| {
            match build(paused.load(Ordering::Relaxed)) {
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
        let (wnd, state, tx, shutdown, paused) = (
            wnd.clone(),
            state.clone(),
            tx.clone(),
            shutdown.clone(),
            paused.clone(),
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
                let mapped = if event.id == *items.pause.id() {
                    Some(TrayEvent::TogglePause)
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
            let want = paused.load(Ordering::Relaxed);
            if items.pause.is_checked() != want {
                items.pause.set_checked(want);
            }
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
