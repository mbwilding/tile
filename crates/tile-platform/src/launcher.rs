//! The app launcher: a search box over everything Start lists (`shell:AppsFolder`), centred
//! on a monitor. It runs on its own thread, and its window is created hidden up front so it
//! opens at once.

use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, Ordering},
        mpsc as std_mpsc,
    },
    thread::{self, JoinHandle},
};

use regex::RegexSet;
use tile_config::{Color, Corners, Launcher as Config, LauncherStyle, SearchPosition};
use tile_core::{
    Command, Rect, calc,
    input::Input,
    search::{self, App, CommandEntry, RankOptions, Usage},
};
use tokio::sync::mpsc::UnboundedSender;
use winsafe::{self as w, co, guard::DeleteObjectGuard, gui, prelude::*};

use crate::monitors::rect;

/// What the launcher asks the daemon to do.
#[derive(Debug, Clone, PartialEq)]
pub enum LauncherEvent {
    /// Start the app with this `shell:AppsFolder` id.
    Launch(String),
    /// Run a command line, like `exec`.
    Run(String),
    /// Run a Tile dispatcher.
    Dispatch(Command),
}

/// A result row.
#[derive(Debug, Clone, PartialEq)]
enum Row {
    /// An index into the apps.
    App(usize),
    Command(CommandEntry),
    /// The value of the search as a calculation.
    Calc(f64),
}

/// What picking a row did.
enum Picked {
    /// Close, then send this to the daemon (after, so a dispatcher acts on the window
    /// that had focus before the launcher).
    Done(LauncherEvent),
    /// Changed the search (completed a dispatcher name): redraw.
    Edited,
    /// Copy this to the clipboard, then close.
    Copy(String),
    Nothing,
}

/// The `launcher` config section with its `hide` patterns compiled and its look resolved.
#[derive(Debug, Clone)]
pub struct LauncherSettings {
    pub config: Config,
    pub hide: RegexSet,
    pub style: LauncherStyle,
}

enum Request {
    Toggle(Option<Rect>),
    Settings(Box<LauncherSettings>),
    Apps(Vec<App>),
    Quit,
}

/// The launcher window, on its own thread.
pub struct Launcher {
    requests: std_mpsc::Sender<Request>,
    hwnd: Arc<AtomicIsize>,
    thread: Option<JoinHandle<()>>,
}

impl Launcher {
    /// Starts the launcher thread. Launch history is kept in `usage_path`.
    pub fn spawn(
        events: UnboundedSender<LauncherEvent>,
        settings: LauncherSettings,
        usage_path: PathBuf,
    ) -> Self {
        let (tx, rx) = std_mpsc::channel();
        let hwnd = Arc::new(AtomicIsize::new(0));
        let thread = thread::Builder::new()
            .name("launcher".into())
            .spawn({
                let (tx, hwnd) = (tx.clone(), hwnd.clone());
                move || run(events, settings, usage_path, rx, tx, hwnd)
            })
            .inspect_err(|e| tracing::error!("failed to start the launcher thread: {e}"))
            .ok();
        Self {
            requests: tx,
            hwnd,
            thread,
        }
    }

    fn send(&self, request: Request) {
        let _ = self.requests.send(request);
        wake(&self.hwnd);
    }

    /// Opens the launcher on the focused window's monitor, or closes it if it is open. With
    /// nothing focused (the desktop or taskbar), it opens in `fallback` (Tile's focused
    /// monitor's work area), or else on the monitor under the pointer.
    pub fn toggle(&self, fallback: Option<Rect>) {
        self.send(Request::Toggle(fallback));
    }

    pub fn set_settings(&self, settings: LauncherSettings) {
        self.send(Request::Settings(Box::new(settings)));
    }

    /// Closes the window and stops the thread.
    pub fn shutdown(&mut self) {
        self.send(Request::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Has the window in `hwnd` (once made) read its requests.
pub(crate) fn wake(hwnd: &AtomicIsize) {
    let hwnd = hwnd.load(Ordering::Acquire);
    if hwnd != 0 {
        tile_hook::post_app(hwnd, 0);
    }
}

/// Everything Start lists, by name. Runs COM on the calling thread.
fn list_apps() -> Vec<App> {
    let _com = w::CoInitializeEx(co::COINIT::APARTMENTTHREADED | co::COINIT::DISABLE_OLE1DDE);
    let list = || -> w::HrResult<Vec<App>> {
        let folder: w::IShellItem =
            w::SHCreateItemFromParsingName("shell:AppsFolder", None::<&w::IBindCtx>)?;
        let items: w::IEnumShellItems =
            folder.BindToHandler(None::<&w::IBindCtx>, &co::BHID::EnumItems)?;
        let mut apps = Vec::new();
        for item in items.iter() {
            let item = item?;
            let (Ok(name), Ok(id)) = (
                item.GetDisplayName(co::SIGDN::NORMALDISPLAY),
                item.GetDisplayName(co::SIGDN::PARENTRELATIVEPARSING),
            ) else {
                continue;
            };
            if !name.trim().is_empty() && !id.is_empty() {
                apps.push(App { name, id });
            }
        }
        Ok(apps)
    };
    let mut apps = list().unwrap_or_else(|e| {
        tracing::warn!("couldn't list apps: {e}");
        Vec::new()
    });
    apps.sort_by_cached_key(|a| (a.name.to_lowercase(), a.id.clone()));
    apps.dedup();
    apps
}

pub(crate) const fn colorref(c: Color) -> w::COLORREF {
    w::COLORREF::from_rgb(c.r, c.g, c.b)
}

/// `a` moved `amount` (0-255) of the way towards `b`.
fn mix(a: Color, b: Color, amount: u16) -> Color {
    let ch = |x: u8, y: u8| ((u16::from(x) * (255 - amount) + u16::from(y) * amount) / 255) as u8;
    Color::rgb(ch(a.r, b.r), ch(a.g, b.g), ch(a.b, b.b))
}

struct Fonts {
    main: DeleteObjectGuard<w::HFONT>,
    small: DeleteObjectGuard<w::HFONT>,
    /// Pixel heights.
    main_px: i32,
    small_px: i32,
}

/// Where things go, in client pixels.
struct Layout {
    search: w::RECT,
    list: w::RECT,
    row: i32,
    icon: i32,
    visible: usize,
}

struct State {
    settings: LauncherSettings,
    events: UnboundedSender<LauncherEvent>,
    /// Everything listed, before `hide`.
    all: Vec<App>,
    apps: Vec<App>,
    loading: bool,
    usage: Usage,
    usage_path: PathBuf,
    query: Input,
    /// A UTF-16 high surrogate waiting for its pair.
    surrogate: Option<u16>,
    results: Vec<Row>,
    selected: usize,
    scroll: usize,
    dpi: u32,
    fonts: Option<Fonts>,
    icons: HashMap<String, Option<tile_hook::Icon>>,
    /// Where the pointer was first seen since opening; moves from there select rows.
    mouse: Option<(i32, i32)>,
}

impl State {
    fn scale(&self, px: i32) -> i32 {
        px * self.dpi as i32 / 96
    }

    fn config(&self) -> &Config {
        &self.settings.config
    }

    fn filter(&mut self) {
        let hide = &self.settings.hide;
        self.apps = self
            .all
            .iter()
            .filter(|a| !hide.is_match(&a.name) && !hide.is_match(&a.id))
            .cloned()
            .collect();
        self.rank();
    }

    fn rank(&mut self) {
        let config = &self.settings.config;
        let options = RankOptions {
            pinned: config.pinned.clone(),
            sort_by_usage: config.sort_by_usage,
            max_results: config.max_results,
        };
        let prefix = config.command_prefix.as_str();
        let query = self.query.text();
        self.results = match query.strip_prefix(prefix) {
            Some(rest) if !prefix.is_empty() => search::commands(rest)
                .into_iter()
                .map(Row::Command)
                .collect(),
            _ => {
                let sum = config
                    .calculator
                    .then(|| calc::calculation(query))
                    .flatten();
                sum.map(Row::Calc)
                    .into_iter()
                    .chain(
                        search::rank(query, &self.apps, &self.usage, &options)
                            .into_iter()
                            .map(Row::App),
                    )
                    .collect()
            }
        };
        self.selected = 0;
        self.scroll = 0;
    }

    fn fonts(&mut self) -> &Fonts {
        if self.fonts.is_none() {
            let config = &self.settings.config;
            let main_px = (config.font_size.clamp(6, 48) * self.dpi / 72) as i32;
            let small_px = main_px * 4 / 5;
            let font = |px: i32| {
                let mut lf = w::LOGFONT::new_face(-px, &self.settings.style.font);
                lf.lfQuality = co::QUALITY::CLEARTYPE;
                w::HFONT::CreateFontIndirect(&lf)
            };
            match (font(main_px), font(small_px)) {
                (Ok(main), Ok(small)) => {
                    self.fonts = Some(Fonts {
                        main,
                        small,
                        main_px,
                        small_px,
                    });
                }
                (Err(e), _) | (_, Err(e)) => {
                    tracing::warn!("launcher font `{}`: {e}", self.settings.style.font);
                }
            }
        }
        self.fonts.get_or_insert_with(|| Fonts {
            main: w::HFONT::CreateFontIndirect(&w::LOGFONT::default())
                .expect("the default font exists"),
            small: w::HFONT::CreateFontIndirect(&w::LOGFONT::default())
                .expect("the default font exists"),
            main_px: 16,
            small_px: 13,
        })
    }

    fn layout(&mut self, client: w::RECT) -> Layout {
        self.fonts();
        let fonts = self.fonts.take().expect("fonts were just made");
        let layout = self.layout_with(&fonts, client);
        self.fonts = Some(fonts);
        layout
    }

    fn layout_with(&self, fonts: &Fonts, client: w::RECT) -> Layout {
        let (main_px, small_px) = (fonts.main_px, fonts.small_px);
        let config = &self.settings.config;
        let pad = self.scale(12);
        let gap = self.scale(8);
        let search_h = main_px + self.scale(24);
        let (row, icon) = if config.compact {
            (main_px + self.scale(16), self.scale(20))
        } else {
            (main_px + small_px + self.scale(22), self.scale(32))
        };
        let icon = if config.show_icons { icon } else { 0 };
        let (left, right) = (client.left + pad, client.right - pad);
        let (search, list) = match config.search_position {
            SearchPosition::Top => {
                let search = w::RECT {
                    left,
                    top: client.top + pad,
                    right,
                    bottom: client.top + pad + search_h,
                };
                let list = w::RECT {
                    left,
                    top: search.bottom + gap,
                    right,
                    bottom: client.bottom - pad,
                };
                (search, list)
            }
            SearchPosition::Bottom => {
                let search = w::RECT {
                    left,
                    top: client.bottom - pad - search_h,
                    right,
                    bottom: client.bottom - pad,
                };
                let list = w::RECT {
                    left,
                    top: client.top + pad,
                    right,
                    bottom: search.top - gap,
                };
                (search, list)
            }
        };
        let visible = ((list.bottom - list.top) / row.max(1)).max(1) as usize;
        Layout {
            search,
            list,
            row,
            icon,
            visible,
        }
    }

    fn select(&mut self, index: isize, visible: usize) {
        if self.results.is_empty() {
            return;
        }
        let last = self.results.len() as isize - 1;
        self.selected = index.clamp(0, last) as usize;
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible {
            self.scroll = self.selected + 1 - visible;
        }
    }

    fn scroll_by(&mut self, rows: isize, visible: usize) {
        let max = self.results.len().saturating_sub(visible) as isize;
        self.scroll = (self.scroll as isize + rows).clamp(0, max) as usize;
    }

    fn row_at(&self, layout: &Layout, x: i32, y: i32) -> Option<usize> {
        let list = layout.list;
        if x < list.left || x >= list.right || y < list.top || y >= list.bottom {
            return None;
        }
        let index = self.scroll + ((y - list.top) / layout.row) as usize;
        (index < self.results.len() && index < self.scroll + layout.visible).then_some(index)
    }

    /// Picks the selected row: what to ask the daemon (noting an app launch), or completes a
    /// dispatcher's name.
    fn pick(&mut self) -> Picked {
        let event = match self.results.get(self.selected).cloned() {
            None => return Picked::Nothing,
            Some(Row::Calc(value)) => return Picked::Copy(calc::format(value)),
            Some(Row::App(i)) => {
                let id = self.apps[i].id.clone();
                if self.config().track_usage {
                    self.usage.record(&id);
                    save_usage(&self.usage_path, &self.usage);
                }
                LauncherEvent::Launch(id)
            }
            Some(Row::Command(CommandEntry::Dispatch(command))) => LauncherEvent::Dispatch(command),
            Some(Row::Command(CommandEntry::Name(name))) => match name.parse() {
                Ok(command) => LauncherEvent::Dispatch(command),
                Err(_) => return Picked::Nothing,
            },
            Some(Row::Command(CommandEntry::Shell(line))) => LauncherEvent::Run(line),
            Some(Row::Command(CommandEntry::Complete { name, .. })) => {
                let text = format!("{}{name} ", self.config().command_prefix);
                self.query.set(text);
                self.rank();
                return Picked::Edited;
            }
        };
        Picked::Done(event)
    }

    /// The row's title, subtitle and app id (for its icon).
    fn describe(&self, row: &Row) -> (String, String, Option<String>) {
        match row {
            Row::App(i) => {
                let app = &self.apps[*i];
                (app.name.clone(), app.id.clone(), Some(app.id.clone()))
            }
            Row::Command(CommandEntry::Dispatch(command)) => {
                (command.to_string(), "Tile dispatcher".into(), None)
            }
            Row::Command(CommandEntry::Name(name)) => {
                ((*name).to_owned(), "Tile dispatcher".into(), None)
            }
            Row::Command(CommandEntry::Complete { name, args }) => (
                format!("{name} {args}"),
                "Tile dispatcher: Enter to add the argument".into(),
                None,
            ),
            Row::Command(CommandEntry::Shell(line)) => (line.clone(), "Run command".into(), None),
            Row::Calc(value) => (
                format!("= {}", calc::format(*value)),
                "Enter to copy".into(),
                None,
            ),
        }
    }

    fn icon(&mut self, id: &str, size: i32) -> Option<&tile_hook::Icon> {
        self.icons
            .entry(id.to_owned())
            .or_insert_with(|| tile_hook::shell_icon(&format!("shell:AppsFolder\\{id}"), size))
            .as_ref()
    }
}

fn load_usage(path: &PathBuf) -> Usage {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_usage(path: &PathBuf, usage: &Usage) {
    let result = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| {
            fs::write(
                path,
                serde_json::to_string_pretty(usage).unwrap_or_default(),
            )
        });
    if let Err(e) = result {
        tracing::warn!("couldn't save {}: {e}", path.display());
    }
}

/// The work area to open in: the focused window's monitor, then `fallback`, then the
/// pointer's monitor.
fn open_area(fallback: Option<Rect>) -> Option<Rect> {
    let work_area = |monitor: w::HMONITOR| monitor.GetMonitorInfo().ok().map(|i| rect(i.rcWork));
    // The desktop and taskbar have focus on an empty monitor, but say nothing about which.
    let focused = w::HWND::GetForegroundWindow()
        .filter(|hwnd| !crate::is_shell_window(hwnd.ptr() as isize))
        .and_then(|hwnd| work_area(hwnd.MonitorFromWindow(co::MONITOR::DEFAULTTONEAREST)));
    focused.or(fallback).or_else(|| {
        work_area(w::HMONITOR::MonitorFromPoint(
            w::GetCursorPos().unwrap_or_default(),
            co::MONITOR::DEFAULTTONEAREST,
        ))
    })
}

pub(crate) const fn corner_preference(corners: Corners) -> co::DWMWCP {
    match corners {
        Corners::Round => co::DWMWCP::ROUND,
        Corners::Small => co::DWMWCP::ROUNDSMALL,
        Corners::Square => co::DWMWCP::DONOTROUND,
    }
}

fn style_window(hwnd: &w::HWND, settings: &LauncherSettings) {
    let (config, style) = (&settings.config, &settings.style);
    let corners = corner_preference(style.corners);
    let _ = hwnd.DwmSetWindowAttribute(w::DwmAttr::WindowCornerPreference(corners));
    let _ = hwnd.DwmSetWindowAttribute(w::DwmAttr::BorderColor(colorref(style.border)));
    let alpha = (config.opacity.clamp(0.1, 1.0) * 255.0).round() as u8;
    if let Err(e) =
        hwnd.SetLayeredWindowAttributes(w::COLORREF::from_rgb(0, 0, 0), alpha, co::LWA::ALPHA)
    {
        tracing::warn!("launcher opacity: {e}");
    }
}

fn run(
    events: UnboundedSender<LauncherEvent>,
    settings: LauncherSettings,
    usage_path: PathBuf,
    requests: std_mpsc::Receiver<Request>,
    requests_tx: std_mpsc::Sender<Request>,
    hwnd_slot: Arc<AtomicIsize>,
) {
    // For the icons, which come from the shell.
    let _com = w::CoInitializeEx(co::COINIT::APARTMENTTHREADED | co::COINIT::DISABLE_OLE1DDE);
    let wnd = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Tile launcher",
        class_name: "tile-launcher",
        class_bg_brush: gui::Brush::None,
        size: (settings.config.width as i32, settings.config.height as i32),
        style: co::WS::POPUP | co::WS::CLIPCHILDREN,
        ex_style: co::WS_EX::TOOLWINDOW | co::WS_EX::TOPMOST | co::WS_EX::LAYERED,
        // Dialog handling would swallow typed characters (and Tab) as mnemonics.
        process_dlg_msgs: false,
        ..Default::default()
    });
    let usage = load_usage(&usage_path);
    let state = Rc::new(RefCell::new(State {
        settings,
        events,
        all: Vec::new(),
        apps: Vec::new(),
        loading: true,
        usage,
        usage_path,
        query: Input::default(),
        surrogate: None,
        results: Vec::new(),
        selected: 0,
        scroll: 0,
        dpi: 96,
        fonts: None,
        icons: HashMap::new(),
        mouse: None,
    }));
    let listing = Arc::new(AtomicBool::new(false));
    let refresh = {
        let (tx, hwnd_slot, listing) = (requests_tx, hwnd_slot.clone(), listing);
        move || {
            if listing.swap(true, Ordering::AcqRel) {
                return;
            }
            let (tx, hwnd_slot, listing) = (tx.clone(), hwnd_slot.clone(), listing.clone());
            let spawned = thread::Builder::new()
                .name("launcher apps".into())
                .spawn(move || {
                    let apps = list_apps();
                    listing.store(false, Ordering::Release);
                    let _ = tx.send(Request::Apps(apps));
                    wake(&hwnd_slot);
                });
            if let Err(e) = spawned {
                tracing::warn!("couldn't list apps: {e}");
            }
        }
    };

    wnd.on().wm_create({
        let (wnd, state, refresh, hwnd_slot) = (
            wnd.clone(),
            state.clone(),
            refresh.clone(),
            hwnd_slot.clone(),
        );
        move |_| {
            style_window(wnd.hwnd(), &state.borrow().settings);
            hwnd_slot.store(wnd.hwnd().ptr() as isize, Ordering::Release);
            refresh();
            // Requests sent before the window existed.
            tile_hook::post_app(wnd.hwnd().ptr() as isize, 0);
            Ok(0)
        }
    });

    let hide = {
        let wnd = wnd.clone();
        move || {
            wnd.hwnd().ShowWindow(co::SW::HIDE);
        }
    };

    let toggle = {
        let (wnd, state, hide, refresh) = (wnd.clone(), state.clone(), hide.clone(), refresh);
        move |fallback: Option<Rect>| {
            let hwnd = wnd.hwnd();
            if hwnd.IsWindowVisible() {
                hide();
                return;
            }
            let Some(area) = open_area(fallback) else {
                return;
            };
            let mut s = state.borrow_mut();
            let (cx, cy) = area.center();
            let dpi = tile_hook::monitor_dpi(cx, cy);
            if dpi != s.dpi {
                s.dpi = dpi;
                s.fonts = None;
                s.icons.clear();
            }
            let w = s
                .scale(s.config().width.clamp(200, 4000) as i32)
                .min(area.w);
            let h = s
                .scale(s.config().height.clamp(100, 4000) as i32)
                .min(area.h);
            s.query.clear();
            s.surrogate = None;
            s.mouse = None;
            s.rank();
            drop(s);
            let placed = hwnd.SetWindowPos(
                w::HwndPlace::Place(co::HWND_PLACE::TOPMOST),
                w::POINT::with(cx - w / 2, cy - h / 2),
                w::SIZE::with(w, h),
                co::SWP::SHOWWINDOW,
            );
            if let Err(e) = placed {
                tracing::warn!("couldn't show the launcher: {e}");
                return;
            }
            let _ = hwnd.InvalidateRect(None, false);
            if !crate::windows::focus_hwnd(hwnd) {
                tracing::debug!("the launcher didn't get focus");
            }
            refresh();
        }
    };

    wnd.on().wm(co::WM::APP, {
        let (wnd, state) = (wnd.clone(), state.clone());
        move |_| {
            while let Ok(request) = requests.try_recv() {
                match request {
                    Request::Toggle(fallback) => toggle(fallback),
                    Request::Settings(settings) => {
                        let mut s = state.borrow_mut();
                        s.settings = *settings;
                        s.fonts = None;
                        s.icons.clear();
                        s.filter();
                        style_window(wnd.hwnd(), &s.settings);
                        let _ = wnd.hwnd().InvalidateRect(None, false);
                    }
                    Request::Apps(apps) => {
                        let mut s = state.borrow_mut();
                        s.loading = false;
                        if apps != s.all {
                            s.all = apps;
                            s.filter();
                            let _ = wnd.hwnd().InvalidateRect(None, false);
                        }
                    }
                    Request::Quit => {
                        state.borrow_mut().icons.clear();
                        w::PostQuitMessage(0);
                    }
                }
            }
            Ok(0)
        }
    });

    wnd.on().wm_activate({
        let hide = hide.clone();
        move |p| {
            if p.event == co::WA::INACTIVE {
                hide();
            }
            Ok(())
        }
    });

    wnd.on().wm_paint({
        let (wnd, state) = (wnd.clone(), state.clone());
        move || {
            let hwnd = wnd.hwnd();
            let hdc = hwnd.BeginPaint()?;
            let client = hwnd.GetClientRect()?;
            let (w, h) = (client.right - client.left, client.bottom - client.top);
            let mem = hdc.CreateCompatibleDC()?;
            let bitmap = hdc.CreateCompatibleBitmap(w, h)?;
            let _bitmap = mem.SelectObject(&*bitmap)?;
            paint(&mut state.borrow_mut(), &mem, client)?;
            hdc.BitBlt(
                w::POINT::default(),
                w::SIZE::with(w, h),
                &mem,
                w::POINT::default(),
                co::ROP::SRCCOPY,
            )?;
            Ok(())
        }
    });

    // Does what picking the selected row asks for.
    let act = {
        let (wnd, hide) = (wnd.clone(), hide.clone());
        move |state: &Rc<RefCell<State>>| {
            let mut s = state.borrow_mut();
            match s.pick() {
                Picked::Done(event) => {
                    let events = s.events.clone();
                    drop(s);
                    hide();
                    let _ = events.send(event);
                }
                Picked::Copy(text) => {
                    drop(s);
                    if let Err(e) = copy(wnd.hwnd(), &text) {
                        tracing::warn!("couldn't copy `{text}`: {e}");
                    }
                    hide();
                }
                Picked::Edited => {
                    drop(s);
                    let _ = wnd.hwnd().InvalidateRect(None, false);
                }
                Picked::Nothing => {}
            }
        }
    };

    wnd.on().wm_key_down({
        let (wnd, state, hide, act) = (wnd.clone(), state.clone(), hide.clone(), act.clone());
        move |p| {
            let ctrl = w::GetKeyState(co::VK::CONTROL).0;
            let shift = w::GetKeyState(co::VK::SHIFT).0;
            let client = wnd.hwnd().GetClientRect()?;
            let mut s = state.borrow_mut();
            let visible = s.layout(client).visible;
            let page = visible as isize;
            let selected = s.selected as isize;
            let mut edited = false;
            match p.vkey_code {
                co::VK::ESCAPE => {
                    drop(s);
                    hide();
                    return Ok(());
                }
                co::VK::RETURN => {
                    drop(s);
                    act(&state);
                    return Ok(());
                }
                co::VK::UP => s.select(selected - 1, visible),
                co::VK::DOWN => s.select(selected + 1, visible),
                co::VK::TAB if shift => s.select(selected - 1, visible),
                co::VK::TAB => s.select(selected + 1, visible),
                co::VK::PRIOR => s.select(selected - page, visible),
                co::VK::NEXT => s.select(selected + page, visible),
                co::VK::CHAR_K | co::VK::CHAR_P if ctrl => s.select(selected - 1, visible),
                co::VK::CHAR_J | co::VK::CHAR_N if ctrl => s.select(selected + 1, visible),
                co::VK::LEFT if ctrl => _ = s.query.word_left(),
                co::VK::RIGHT if ctrl => _ = s.query.word_right(),
                co::VK::LEFT => _ = s.query.left(),
                co::VK::RIGHT => _ = s.query.right(),
                co::VK::CHAR_H if ctrl => _ = s.query.left(),
                co::VK::CHAR_L if ctrl => _ = s.query.right(),
                co::VK::HOME => _ = s.query.home(),
                co::VK::END => _ = s.query.end(),
                co::VK::BACK | co::VK::CHAR_W if ctrl => edited = s.query.delete_word(),
                co::VK::CHAR_U if ctrl => edited = s.query.delete_to_start(),
                co::VK::BACK => edited = s.query.backspace(),
                co::VK::DELETE => edited = s.query.delete(),
                _ => return Ok(()),
            }
            if edited {
                s.rank();
            }
            drop(s);
            let _ = wnd.hwnd().InvalidateRect(None, false);
            Ok(())
        }
    });

    wnd.on().wm_char({
        let (wnd, state) = (wnd.clone(), state.clone());
        move |p| {
            let code = p.char_code;
            let mut s = state.borrow_mut();
            let c = match code {
                0xD800..=0xDBFF => {
                    s.surrogate = Some(code);
                    return Ok(());
                }
                0xDC00..=0xDFFF => {
                    let Some(high) = s.surrogate.take() else {
                        return Ok(());
                    };
                    char::decode_utf16([high, code]).next().and_then(Result::ok)
                }
                // Control characters: their keys are handled in `wm_key_down`.
                ..0x20 | 0x7F => None,
                _ => char::from_u32(u32::from(code)),
            };
            let Some(c) = c else {
                return Ok(());
            };
            s.query.insert(c);
            s.rank();
            drop(s);
            let _ = wnd.hwnd().InvalidateRect(None, false);
            Ok(())
        }
    });

    wnd.on().wm_mouse_move({
        let (wnd, state) = (wnd.clone(), state.clone());
        move |p| {
            let mut s = state.borrow_mut();
            if s.config().ignore_mouse {
                return Ok(());
            }
            let at = (p.coords.x, p.coords.y);
            // Windows sends a move when the window opens under a still pointer; only real
            // moves select.
            match s.mouse {
                None => {
                    s.mouse = Some(at);
                    return Ok(());
                }
                Some(first) if first == at => return Ok(()),
                Some(_) => {}
            }
            let layout = s.layout(wnd.hwnd().GetClientRect()?);
            if let Some(row) = s.row_at(&layout, at.0, at.1)
                && row != s.selected
            {
                s.selected = row;
                drop(s);
                let _ = wnd.hwnd().InvalidateRect(None, false);
            }
            Ok(())
        }
    });

    wnd.on().wm_l_button_down({
        let (wnd, state) = (wnd.clone(), state.clone());
        move |p| {
            let mut s = state.borrow_mut();
            if s.config().ignore_mouse {
                return Ok(());
            }
            let layout = s.layout(wnd.hwnd().GetClientRect()?);
            if let Some(row) = s.row_at(&layout, p.coords.x, p.coords.y) {
                s.selected = row;
                drop(s);
                act(&state);
            }
            Ok(())
        }
    });

    wnd.on().wm_mouse_wheel({
        let (wnd, state) = (wnd.clone(), state.clone());
        move |p| {
            let mut s = state.borrow_mut();
            if s.config().ignore_mouse {
                return Ok(());
            }
            let visible = s.layout(wnd.hwnd().GetClientRect()?).visible;
            let rows = -isize::from(p.wheel_distance) / 120 * 3;
            s.scroll_by(rows, visible);
            drop(s);
            let _ = wnd.hwnd().InvalidateRect(None, false);
            Ok(())
        }
    });

    if let Err(e) = wnd.run_main(Some(co::SW::HIDE)) {
        tracing::error!("launcher message loop failed: {e}");
    }
    hwnd_slot.store(0, Ordering::Release);
}

/// Puts `text` on the clipboard.
fn copy(hwnd: &w::HWND, text: &str) -> w::SysResult<()> {
    let clipboard = hwnd.OpenClipboard()?;
    clipboard.EmptyClipboard()?;
    let bytes: Vec<u8> = text
        .encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect();
    clipboard.SetClipboardData(co::CF::UNICODETEXT, &bytes)
}

fn fill_round(dc: &w::HDC, bounds: w::RECT, radius: i32, color: Color) -> w::SysResult<()> {
    let brush = w::HBRUSH::CreateSolidBrush(colorref(color))?;
    let pen = w::HPEN::GetStockObject(co::STOCK_PEN::NULL)?;
    let _brush = dc.SelectObject(&*brush)?;
    let _pen = dc.SelectObject(&pen)?;
    // RoundRect leaves out the bottom and right edges with a null pen.
    let bounds = w::RECT {
        right: bounds.right + 1,
        bottom: bounds.bottom + 1,
        ..bounds
    };
    dc.RoundRect(bounds, w::SIZE::with(radius, radius))
}

fn text(
    dc: &w::HDC,
    font: &w::HFONT,
    color: Color,
    s: &str,
    mut bounds: w::RECT,
    flags: co::DT,
) -> w::SysResult<()> {
    let _font = dc.SelectObject(font)?;
    dc.SetTextColor(colorref(color))?;
    dc.DrawText(
        s,
        &mut bounds,
        flags | co::DT::SINGLELINE | co::DT::NOPREFIX | co::DT::END_ELLIPSIS,
    )?;
    Ok(())
}

fn paint(s: &mut State, dc: &w::HDC, client: w::RECT) -> w::SysResult<()> {
    s.fonts();
    let fonts = s.fonts.take().expect("fonts were just made");
    let result = paint_with(s, &fonts, dc, client);
    s.fonts = Some(fonts);
    result
}

fn paint_with(s: &mut State, fonts: &Fonts, dc: &w::HDC, client: w::RECT) -> w::SysResult<()> {
    let config = s.config().clone();
    let style = s.settings.style.clone();
    let layout = s.layout_with(fonts, client);
    let radius = s.scale(16);
    let pad = s.scale(12);
    let background = w::HBRUSH::CreateSolidBrush(colorref(style.background))?;
    dc.FillRect(client, &background)?;
    dc.SetBkMode(co::BKMODE::TRANSPARENT)?;

    // The search box.
    let field = mix(style.background, style.foreground, 20);
    fill_round(dc, layout.search, radius, field)?;
    let mut inner = layout.search;
    inner.left += pad;
    inner.right -= pad;
    let (main, small) = (&*fonts.main, &*fonts.small);
    let left = co::DT::LEFT | co::DT::VCENTER;
    if s.query.text().is_empty() {
        let placeholder = config.placeholder_text();
        text(dc, main, style.dimmed, &placeholder, inner, left)?;
    } else {
        text(dc, main, style.foreground, s.query.text(), inner, left)?;
    }
    let typed = {
        let _font = dc.SelectObject(main)?;
        dc.GetTextExtentPoint32(s.query.before_caret())?.cx
    };
    let caret_x = (inner.left + typed + 1).min(inner.right);
    let caret_h = fonts.main_px;
    let mid = (inner.top + inner.bottom) / 2;
    let caret = w::RECT {
        left: caret_x,
        top: mid - caret_h / 2,
        right: caret_x + s.scale(2).max(1),
        bottom: mid + caret_h / 2,
    };
    let caret_brush = w::HBRUSH::CreateSolidBrush(colorref(style.selected))?;
    dc.FillRect(caret, &caret_brush)?;

    // The results.
    if s.results.is_empty() {
        let message = if s.loading {
            "Loading apps…"
        } else {
            "No matches"
        };
        text(
            dc,
            main,
            style.dimmed,
            message,
            layout.list,
            co::DT::CENTER | co::DT::VCENTER,
        )?;
        return Ok(());
    }
    let end = (s.scroll + layout.visible).min(s.results.len());
    for (slot, index) in (s.scroll..end).enumerate() {
        let (title, subtitle, icon_id) = s.describe(&s.results[index]);
        let top = layout.list.top + slot as i32 * layout.row;
        let row = w::RECT {
            left: layout.list.left,
            top,
            right: layout.list.right,
            bottom: top + layout.row,
        };
        let selected = index == s.selected;
        let (fg, dim) = if selected {
            fill_round(dc, row, radius, style.selected)?;
            (style.selected_foreground, style.selected_foreground)
        } else {
            (style.foreground, style.dimmed)
        };
        let mut x = row.left + pad;
        if layout.icon > 0 {
            let y = top + (layout.row - layout.icon) / 2;
            if let Some(icon) = icon_id.and_then(|id| s.icon(&id, layout.icon)) {
                icon.draw(dc.ptr() as isize, x, y, layout.icon);
            }
            x += layout.icon + pad;
        }
        let bounds = w::RECT {
            left: x,
            right: row.right - pad,
            ..row
        };
        if config.compact {
            text(dc, main, fg, &title, bounds, left)?;
        } else {
            let (main_px, small_px) = (fonts.main_px, fonts.small_px);
            let spacing = s.scale(2);
            let block = main_px + spacing + small_px;
            let name_top = top + (layout.row - block) / 2;
            let name = w::RECT {
                top: name_top,
                bottom: name_top + main_px + spacing,
                ..bounds
            };
            let id = w::RECT {
                top: name.bottom,
                bottom: name.bottom + small_px + spacing,
                ..bounds
            };
            text(dc, main, fg, &title, name, co::DT::LEFT | co::DT::TOP)?;
            text(dc, small, dim, &subtitle, id, co::DT::LEFT | co::DT::TOP)?;
        }
    }
    Ok(())
}
