//! Finding programs the way `start` does, so a hotkey can fall back to another one.

use std::{env, fs, path::Path};

use winsafe::{self as w, co};

const APP_PATHS: &str = r"Software\Microsoft\Windows\CurrentVersion\App Paths";

/// Whether `start` can find `program`: a path that exists, a file on `PATH` (app execution
/// aliases like `wt.exe` included), or a program registered under App Paths (like
/// `chrome.exe`). `PATHEXT` extensions are tried when it has none. URLs and other protocol
/// targets count as installed.
pub fn installed(program: &str) -> bool {
    let path = Path::new(program);
    if !path.is_absolute() && program.contains(':') {
        return true;
    }
    let names = with_extensions(program);
    // Aliases are reparse points that can't be followed, so don't follow them.
    let exists = |p: &Path| fs::symlink_metadata(p).is_ok();
    if path.components().count() > 1 {
        return names.iter().any(|n| exists(Path::new(n)));
    }
    let on_path = env::var_os("PATH").is_some_and(|dirs| {
        env::split_paths(&dirs).any(|dir| names.iter().any(|n| exists(&dir.join(n))))
    });
    on_path || names.iter().any(|n| registered(n))
}

fn with_extensions(program: &str) -> Vec<String> {
    if Path::new(program).extension().is_some() {
        return vec![program.to_owned()];
    }
    let pathext = env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    std::iter::once(program.to_owned())
        .chain(
            pathext
                .split(';')
                .filter(|e| !e.is_empty())
                .map(|e| format!("{program}{e}")),
        )
        .collect()
}

fn registered(name: &str) -> bool {
    let key = format!(r"{APP_PATHS}\{name}");
    [w::HKEY::CURRENT_USER, w::HKEY::LOCAL_MACHINE]
        .iter()
        .any(|root| {
            root.RegOpenKeyEx(Some(&key), co::REG_OPTION::default(), co::KEY::READ)
                .is_ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_programs() {
        assert!(installed("cmd.exe"));
        assert!(installed("cmd"));
        assert!(installed(r"C:\Windows\System32\cmd.exe"));
        assert!(installed("https://example.com"));
        assert!(!installed("tile-no-such-program.exe"));
        assert!(!installed(r"C:\tile-no-such-dir\x.exe"));
    }
}
