use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{geometry::Direction, layout::LayoutKind};

/// A Hyprland-style dispatcher. Shared by keybinds, IPC and `tilingctl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    Exec(String),
    KillActive,
    ToggleFloating,
    Fullscreen,
    MoveFocus(Direction),
    MoveWindow(Direction),
    /// Grow (or shrink, if negative) the active window by pixels.
    ResizeActive(i32, i32),
    SwapWithMaster,
    ToggleSplit,
    Workspace(u8),
    MoveToWorkspace(u8),
    MoveToWorkspaceSilent(u8),
    FocusMonitor(Direction),
    Layout(LayoutKind),
    CycleLayout,
    /// Scrolling layout: set the focused column's width as a fraction of the monitor.
    ScrollerProportion(f32),
    CycleProportion,
    ConsumeWindow(Direction),
    ExpelWindow,
    CenterColumn,
    Reload,
    TogglePause,
    Exit,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("empty command")]
    Empty,
    #[error("unknown dispatcher `{0}`")]
    Unknown(String),
    #[error("`{0}` expects an argument: {1}")]
    MissingArg(String, &'static str),
    #[error("invalid argument `{1}` for `{0}`: expected {2}")]
    BadArg(String, String, &'static str),
}

fn parse_direction(name: &str, arg: Option<&str>) -> Result<Direction, ParseError> {
    let arg = arg.ok_or_else(|| ParseError::MissingArg(name.into(), "l|r|u|d"))?;
    match arg.to_ascii_lowercase().as_str() {
        "l" | "left" => Ok(Direction::Left),
        "r" | "right" => Ok(Direction::Right),
        "u" | "up" | "t" | "top" => Ok(Direction::Up),
        "d" | "down" | "b" | "bottom" => Ok(Direction::Down),
        _ => Err(ParseError::BadArg(name.into(), arg.into(), "l|r|u|d")),
    }
}

fn parse_workspace(name: &str, arg: Option<&str>) -> Result<u8, ParseError> {
    let arg = arg.ok_or_else(|| ParseError::MissingArg(name.into(), "1-255"))?;
    match arg.parse::<u8>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(ParseError::BadArg(name.into(), arg.into(), "1-255")),
    }
}

impl FromStr for Command {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (name, rest) = match s.split_once(char::is_whitespace) {
            Some((name, rest)) => (name, Some(rest.trim()).filter(|r| !r.is_empty())),
            None => (s, None),
        };
        if name.is_empty() {
            return Err(ParseError::Empty);
        }
        let name = name.to_ascii_lowercase();
        let n = name.as_str();
        Ok(match n {
            "exec" => Self::Exec(
                rest.ok_or_else(|| ParseError::MissingArg(name.clone(), "a command line"))?
                    .to_string(),
            ),
            "killactive" => Self::KillActive,
            "togglefloating" => Self::ToggleFloating,
            "fullscreen" => Self::Fullscreen,
            "movefocus" => Self::MoveFocus(parse_direction(n, rest)?),
            "movewindow" => Self::MoveWindow(parse_direction(n, rest)?),
            "resizeactive" => {
                let rest = rest.ok_or_else(|| ParseError::MissingArg(name.clone(), "dx dy"))?;
                let bad = || ParseError::BadArg(name.clone(), rest.into(), "two integers");
                let mut parts = rest.split_whitespace().map(str::parse::<i32>);
                match (parts.next(), parts.next(), parts.next()) {
                    (Some(Ok(dx)), Some(Ok(dy)), None) => Self::ResizeActive(dx, dy),
                    _ => return Err(bad()),
                }
            }
            "swapwithmaster" => Self::SwapWithMaster,
            "togglesplit" => Self::ToggleSplit,
            "workspace" => Self::Workspace(parse_workspace(n, rest)?),
            "movetoworkspace" => Self::MoveToWorkspace(parse_workspace(n, rest)?),
            "movetoworkspacesilent" => Self::MoveToWorkspaceSilent(parse_workspace(n, rest)?),
            "focusmonitor" => Self::FocusMonitor(parse_direction(n, rest)?),
            "layout" => {
                let arg = rest.ok_or_else(|| {
                    ParseError::MissingArg(name.clone(), "dwindle|master|monocle|scrolling")
                })?;
                Self::Layout(arg.parse().map_err(|()| {
                    ParseError::BadArg(name.clone(), arg.into(), "dwindle|master|monocle|scrolling")
                })?)
            }
            "cyclelayout" => Self::CycleLayout,
            "scroller_proportion" | "scrollerproportion" => {
                let arg = rest.ok_or_else(|| ParseError::MissingArg(name.clone(), "0.1-1.0"))?;
                match arg.parse::<f32>() {
                    Ok(p) if (0.05..=1.0).contains(&p) => Self::ScrollerProportion(p),
                    _ => return Err(ParseError::BadArg(name, arg.into(), "0.05-1.0")),
                }
            }
            "cycleproportion" => Self::CycleProportion,
            "consumewindow" => Self::ConsumeWindow(parse_direction(n, rest)?),
            "expelwindow" => Self::ExpelWindow,
            "centercolumn" => Self::CenterColumn,
            "reload" => Self::Reload,
            "togglepause" => Self::TogglePause,
            "exit" => Self::Exit,
            _ => return Err(ParseError::Unknown(name)),
        })
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Left => "l",
            Self::Right => "r",
            Self::Up => "u",
            Self::Down => "d",
        })
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exec(cmd) => write!(f, "exec {cmd}"),
            Self::KillActive => f.write_str("killactive"),
            Self::ToggleFloating => f.write_str("togglefloating"),
            Self::Fullscreen => f.write_str("fullscreen"),
            Self::MoveFocus(d) => write!(f, "movefocus {d}"),
            Self::MoveWindow(d) => write!(f, "movewindow {d}"),
            Self::ResizeActive(dx, dy) => write!(f, "resizeactive {dx} {dy}"),
            Self::SwapWithMaster => f.write_str("swapwithmaster"),
            Self::ToggleSplit => f.write_str("togglesplit"),
            Self::Workspace(n) => write!(f, "workspace {n}"),
            Self::MoveToWorkspace(n) => write!(f, "movetoworkspace {n}"),
            Self::MoveToWorkspaceSilent(n) => write!(f, "movetoworkspacesilent {n}"),
            Self::FocusMonitor(d) => write!(f, "focusmonitor {d}"),
            Self::Layout(kind) => write!(f, "layout {kind}"),
            Self::CycleLayout => f.write_str("cyclelayout"),
            Self::ScrollerProportion(p) => write!(f, "scroller_proportion {p}"),
            Self::CycleProportion => f.write_str("cycleproportion"),
            Self::ConsumeWindow(d) => write!(f, "consumewindow {d}"),
            Self::ExpelWindow => f.write_str("expelwindow"),
            Self::CenterColumn => f.write_str("centercolumn"),
            Self::Reload => f.write_str("reload"),
            Self::TogglePause => f.write_str("togglepause"),
            Self::Exit => f.write_str("exit"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_round_trips() {
        for input in [
            "exec wt.exe -p \"PowerShell\"",
            "killactive",
            "movefocus l",
            "resizeactive -40 0",
            "workspace 3",
            "movetoworkspacesilent 9",
            "layout scrolling",
            "scroller_proportion 0.5",
            "consumewindow r",
        ] {
            let cmd: Command = input.parse().unwrap();
            assert_eq!(cmd.to_string(), input);
        }
    }

    #[test]
    fn accepts_long_directions_and_case() {
        assert_eq!("MoveFocus Left".parse(), Ok(Command::MoveFocus(Direction::Left)));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!("".parse::<Command>(), Err(ParseError::Empty)));
        assert!(matches!("nope".parse::<Command>(), Err(ParseError::Unknown(_))));
        assert!(matches!("workspace 0".parse::<Command>(), Err(ParseError::BadArg(..))));
        assert!(matches!("movefocus".parse::<Command>(), Err(ParseError::MissingArg(..))));
        assert!(matches!("resizeactive 1".parse::<Command>(), Err(ParseError::BadArg(..))));
    }
}
