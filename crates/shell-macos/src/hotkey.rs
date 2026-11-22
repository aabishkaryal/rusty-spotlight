//! The global shortcut (FR-1): read from `hotkey` in config.toml, registered
//! through Carbon, mirrored by the key tap, and changeable while running:
//! Settings records a new one and applies it at once.
//!
//! A shortcut is one or more of Command, Option, Control, Shift, plus Space,
//! a letter or a digit: `cmd+space`, `option+space`, `ctrl+shift+k`.

use std::cell::RefCell;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use objc2::MainThreadMarker;
use objc2_app_kit::NSEventModifierFlags;

use crate::ui::event_log;

/// Keys a shortcut can end in: config name, Carbon code, macOS key code, shown name.
const KEYS: &[(&str, Code, u16, &str)] = &[
    ("space", Code::Space, 49, "Space"),
    ("a", Code::KeyA, 0, "A"), ("b", Code::KeyB, 11, "B"), ("c", Code::KeyC, 8, "C"), ("d", Code::KeyD, 2, "D"),
    ("e", Code::KeyE, 14, "E"), ("f", Code::KeyF, 3, "F"), ("g", Code::KeyG, 5, "G"), ("h", Code::KeyH, 4, "H"),
    ("i", Code::KeyI, 34, "I"), ("j", Code::KeyJ, 38, "J"), ("k", Code::KeyK, 40, "K"), ("l", Code::KeyL, 37, "L"),
    ("m", Code::KeyM, 46, "M"), ("n", Code::KeyN, 45, "N"), ("o", Code::KeyO, 31, "O"), ("p", Code::KeyP, 35, "P"),
    ("q", Code::KeyQ, 12, "Q"), ("r", Code::KeyR, 15, "R"), ("s", Code::KeyS, 1, "S"), ("t", Code::KeyT, 17, "T"),
    ("u", Code::KeyU, 32, "U"), ("v", Code::KeyV, 9, "V"), ("w", Code::KeyW, 13, "W"), ("x", Code::KeyX, 7, "X"),
    ("y", Code::KeyY, 16, "Y"), ("z", Code::KeyZ, 6, "Z"),
    ("0", Code::Digit0, 29, "0"), ("1", Code::Digit1, 18, "1"), ("2", Code::Digit2, 19, "2"), ("3", Code::Digit3, 20, "3"),
    ("4", Code::Digit4, 21, "4"), ("5", Code::Digit5, 23, "5"), ("6", Code::Digit6, 22, "6"), ("7", Code::Digit7, 26, "7"),
    ("8", Code::Digit8, 28, "8"), ("9", Code::Digit9, 25, "9"),
];

/// Modifiers in the order macOS writes them, with config and shown names.
const MODS: &[(Modifiers, NSEventModifierFlags, &str, &str)] = &[
    (Modifiers::CONTROL, NSEventModifierFlags::Control, "ctrl", "Control"),
    (Modifiers::ALT, NSEventModifierFlags::Option, "opt", "Option"),
    (Modifiers::SHIFT, NSEventModifierFlags::Shift, "shift", "Shift"),
    (Modifiers::META, NSEventModifierFlags::Command, "cmd", "Command"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Shortcut {
    pub mods: Modifiers,
    code: Code,
    /// macOS virtual key code, for the key tap.
    pub keycode: u16,
    /// The key alone, as a menu's key equivalent: " " or "k".
    pub key: String,
    /// "Command-Space".
    pub name: String,
    /// "cmd+space", as config.toml stores it.
    pub text: String,
}

impl Shortcut {
    pub fn menu_mods(&self) -> NSEventModifierFlags {
        MODS.iter().filter(|m| self.mods.contains(m.0)).fold(NSEventModifierFlags::empty(), |f, m| f | m.1)
    }
}

fn build(mods: Modifiers, key: &str) -> Option<Shortcut> {
    let &(config_key, code, keycode, shown) = KEYS.iter().find(|k| k.0 == key)?;
    if mods.is_empty() || mods == Modifiers::SHIFT {
        return None; // a bare key or Shift+key would type letters
    }
    let used: Vec<_> = MODS.iter().filter(|m| mods.contains(m.0)).collect();
    let mut name: Vec<&str> = used.iter().map(|m| m.3).collect();
    name.push(shown);
    let mut text: Vec<&str> = used.iter().map(|m| m.2).collect();
    text.push(config_key);
    Some(Shortcut {
        mods,
        code,
        keycode,
        key: if config_key == "space" { " ".into() } else { config_key.into() },
        name: name.join("-"),
        text: text.join("+"),
    })
}

/// `cmd+space`, `option+space`, `ctrl+shift+k`.
pub fn parse(text: &str) -> Option<Shortcut> {
    let mut mods = Modifiers::empty();
    let mut key = None;
    for part in text.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "cmd" | "command" | "super" | "meta" => mods |= Modifiers::META,
            "opt" | "option" | "alt" => mods |= Modifiers::ALT,
            "ctrl" | "control" => mods |= Modifiers::CONTROL,
            "shift" => mods |= Modifiers::SHIFT,
            other if key.is_none() => key = Some(other.to_string()),
            _ => return None,
        }
    }
    build(mods, &key?)
}

/// A key press recorded in Settings: its modifier flags and key code.
pub fn from_event(flags: NSEventModifierFlags, keycode: u16) -> Result<Shortcut, String> {
    let mut mods = Modifiers::empty();
    for m in MODS {
        if flags.contains(m.1) {
            mods |= m.0;
        }
    }
    let Some(key) = KEYS.iter().find(|k| k.2 == keycode) else {
        return Err("Use Space, a letter or a digit, with Command, Option or Control.".into());
    };
    build(mods, key.0).ok_or_else(|| "Hold Command, Option or Control as well.".into())
}

struct Registered {
    hotkey: HotKey,
    shortcut: Shortcut,
}

thread_local! {
    static MANAGER: RefCell<Option<GlobalHotKeyManager>> = const { RefCell::new(None) };
    static CURRENT: RefCell<Option<Registered>> = const { RefCell::new(None) };
    /// Why registration failed, when it did.
    static PROBLEM: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Register the shortcut from config.toml (or RS_HOTKEY) and route presses
/// to `on_press`. A failure leaves the app usable from the menu bar.
pub fn init(wanted: &str, on_press: fn()) {
    let shortcut = parse(wanted).unwrap_or_else(|| {
        eprintln!("[rs] hotkey {wanted:?} not understood; using option+space");
        parse("option+space").expect("valid")
    });
    match GlobalHotKeyManager::new() {
        Ok(manager) => MANAGER.set(Some(manager)),
        Err(e) => {
            PROBLEM.set(Some(format!("Shortcuts unavailable: {e}")));
            eprintln!("[rs] shortcuts unavailable: {e}");
            return;
        }
    }
    // The Carbon handler runs on the main thread's event target.
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        let ours = CURRENT.with_borrow(|c| c.as_ref().is_some_and(|r| r.hotkey.id() == event.id));
        if !ours || event.state != HotKeyState::Pressed {
            return;
        }
        if MainThreadMarker::new().is_none() {
            eprintln!("[rs] hotkey event arrived off the main thread; ignored");
            return;
        }
        if crate::keytap::claim() {
            return; // the key tap already acted on this press
        }
        event_log(if crate::keytap::active() { "hotkey pressed (Carbon; the key tap missed it)" } else { "hotkey pressed" });
        on_press();
    }));
    match register(&shortcut) {
        Ok(()) => eprintln!("[rs] Hotkey: {}", shortcut.name),
        Err(e) => eprintln!("[rs] {e}"),
    }
    crate::keytap::start(on_press);
    sync_tap();
}

fn register(shortcut: &Shortcut) -> Result<(), String> {
    let hotkey = HotKey::new(Some(shortcut.mods), shortcut.code);
    let result = MANAGER.with_borrow(|m| match m {
        Some(m) => m.register(hotkey).map_err(|e| e.to_string()),
        None => Err("shortcuts unavailable".into()),
    });
    match result {
        Ok(()) => {
            CURRENT.set(Some(Registered { hotkey, shortcut: shortcut.clone() }));
            PROBLEM.set(None);
            Ok(())
        }
        Err(e) => {
            let reason = format!("{} is unavailable: {e}", shortcut.name);
            PROBLEM.set(Some(reason.clone()));
            Err(reason)
        }
    }
}

fn unregister() {
    if let Some(old) = CURRENT.take() {
        MANAGER.with_borrow(|m| {
            if let Some(m) = m {
                let _ = m.unregister(old.hotkey);
            }
        });
    }
}

fn sync_tap() {
    match current() {
        Some(s) => crate::keytap::watch(s.menu_mods().bits() as u64, s.keycode),
        None => crate::keytap::watch(0, 0),
    }
}

/// Use `shortcut` from now on. On failure the previous one stays.
pub fn change(shortcut: &Shortcut) -> Result<(), String> {
    let previous = current();
    if previous.as_ref() == Some(shortcut) {
        return Ok(());
    }
    unregister();
    let result = register(shortcut);
    if result.is_err() {
        if let Some(p) = &previous {
            let _ = register(p);
        }
    } else {
        event_log(&format!("shortcut is now {}", shortcut.name));
    }
    sync_tap();
    result
}

/// While Settings records a shortcut, the current one must not open the panel.
pub fn pause(paused: bool) {
    if paused {
        let keep = current();
        unregister();
        crate::keytap::watch(0, 0);
        PAUSED.set(keep);
    } else if let Some(s) = PAUSED.take() {
        let _ = register(&s);
        sync_tap();
    }
}

thread_local! {
    static PAUSED: RefCell<Option<Shortcut>> = const { RefCell::new(None) };
}

pub fn current() -> Option<Shortcut> {
    CURRENT.with_borrow(|c| c.as_ref().map(|r| r.shortcut.clone()))
}

/// Why the shortcut is not working, if it is not.
pub fn problem() -> Option<String> {
    PROBLEM.with_borrow(|p| p.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_shortcuts() {
        let s = parse("cmd+space").unwrap();
        assert_eq!((s.name.as_str(), s.text.as_str(), s.keycode, s.key.as_str()), ("Command-Space", "cmd+space", 49, " "));
        let s = parse("Shift + Control + k").unwrap();
        assert_eq!((s.name.as_str(), s.text.as_str(), s.keycode), ("Control-Shift-K", "ctrl+shift+k", 40));
        assert_eq!(parse("option+space").unwrap().text, "opt+space");
        for bad in ["space", "shift+a", "cmd", "cmd+f5", "cmd+space+k"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        let recorded = from_event(NSEventModifierFlags::Command | NSEventModifierFlags::Option, 49).unwrap();
        assert_eq!(recorded.text, "opt+cmd+space");
        assert!(from_event(NSEventModifierFlags::Command, 122).is_err());
        assert!(from_event(NSEventModifierFlags::Shift, 0).is_err());
    }
}
