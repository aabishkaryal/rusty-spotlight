//! A small Spotlight-style launcher. See docs/REQUIREMENTS.md and docs/PLAN.md.
//!
//! Knobs kept while spike S1's manual checks are outstanding:
//!   RS_LEVEL     floating | modal | status | popup (default) | screensaver
//!   RS_BEHAVIOR  a (default: all spaces + fullscreen auxiliary + stationary)
//!                b (can join all applications)   c (move to active space)
//!   RS_MATERIAL  glass (default) | blur
//!   RS_HOTKEY    overrides `hotkey` in config.toml, e.g. `option+space`
//!   RS_DEBUG     any value: log show timings to stderr

mod apps;
mod ask;
mod config;
mod keytap;
mod rates;
mod settings;
mod store;
mod ui;
mod watch;

use std::cell::RefCell;
use std::time::Instant;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEventModifierFlags};

use ui::Controller;

thread_local! {
    static CONTROLLER: RefCell<Option<Retained<Controller>>> = const { RefCell::new(None) };
}

fn diagnostics(args: &[String]) -> bool {
    // rusty-spotlight --apps            list the discovered apps
    // rusty-spotlight --rank <query>    show the ranked results for a query
    if args.len() < 2 || (args[1] != "--apps" && args[1] != "--rank") {
        return false;
    }
    let t = Instant::now();
    let items = apps::scan(&config::load(None).config.aliases);
    let scan_ms = t.elapsed().as_secs_f64() * 1000.0;
    if args[1] == "--apps" {
        for i in &items {
            println!("{}\t{}", i.title, i.id);
        }
    } else {
        let q = args.get(2).map(String::as_str).unwrap_or("");
        let usage = store::load();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        let t = Instant::now();
        let ranked = launcher_core::Ranker::default().rank(q, &items, &usage, now, 8);
        let rank_us = t.elapsed().as_secs_f64() * 1e6;
        for r in &ranked {
            println!("{:?}\t{:.2}\t{}", r.tier, r.weight, items[r.index].title);
        }
        eprintln!("rank: {rank_us:.0} µs over {} apps", items.len());
    }
    eprintln!("scan: {} apps in {scan_ms:.1} ms", items.len());
    true
}

/// `cmd+space`, `option+space`, `ctrl+shift+space`: modifiers, then Space.
fn parse_hotkey(text: &str) -> Option<(Modifiers, String)> {
    let mut mods = Modifiers::empty();
    let mut names = Vec::new();
    let mut key = false;
    for part in text.split('+').map(|p| p.trim().to_lowercase()) {
        match part.as_str() {
            "cmd" | "command" | "super" | "meta" => {
                mods |= Modifiers::META;
                names.push("Command");
            }
            "opt" | "option" | "alt" => {
                mods |= Modifiers::ALT;
                names.push("Option");
            }
            "ctrl" | "control" => {
                mods |= Modifiers::CONTROL;
                names.push("Control");
            }
            "shift" => {
                mods |= Modifiers::SHIFT;
                names.push("Shift");
            }
            "space" => key = true,
            _ => return None,
        }
    }
    (key && !mods.is_empty()).then(|| (mods, format!("{}-Space", names.join("-"))))
}

/// The same modifiers, as a menu draws them beside an item.
fn menu_modifiers(mods: Modifiers) -> NSEventModifierFlags {
    let mut flags = NSEventModifierFlags::empty();
    for (m, f) in [
        (Modifiers::META, NSEventModifierFlags::Command),
        (Modifiers::ALT, NSEventModifierFlags::Option),
        (Modifiers::CONTROL, NSEventModifierFlags::Control),
        (Modifiers::SHIFT, NSEventModifierFlags::Shift),
    ] {
        if mods.contains(m) {
            flags |= f;
        }
    }
    flags
}

fn toggle_panel() {
    // Clone the handle out so no borrow is held while AppKit runs.
    let controller = CONTROLLER.with_borrow(|c| c.clone());
    if let Some(controller) = controller {
        controller.toggle();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if diagnostics(&args) {
        return;
    }

    let mtm = MainThreadMarker::new().expect("must start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    // Menu-bar agent: no Dock icon, never the active app.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller = Controller::new(mtm);
    if args.get(1).map(String::as_str) == Some("--snapshot-settings") {
        settings::snapshot(&controller, args.get(2).map(String::as_str).unwrap_or("."));
        return;
    }

    // Hotkey. A failure here must leave the app usable from the menu bar (FR-1).
    // RS_HOTKEY overrides the configuration file, for testing.
    let wanted = std::env::var("RS_HOTKEY").unwrap_or_else(|_| config::load(None).config.hotkey);
    let (mods, name) = parse_hotkey(&wanted).unwrap_or_else(|| {
        eprintln!("[rs] hotkey {wanted:?} not understood; using option+space");
        (Modifiers::ALT, "Option-Space".to_string())
    });
    let hotkey = HotKey::new(Some(mods), Code::Space);
    let hotkey_id = hotkey.id();
    let manager = GlobalHotKeyManager::new();
    let (registered, hotkey_status) = match &manager {
        Ok(m) => match m.register(hotkey) {
            Ok(()) => (true, format!("Hotkey: {name}")),
            Err(e) => (false, format!("Shortcut {name} unavailable: {e}")),
        },
        Err(e) => (false, format!("Shortcuts unavailable: {e}")),
    };
    eprintln!("[rs] {hotkey_status}");
    // The Carbon handler runs on the main thread's event target.
    GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
        if event.id != hotkey_id || event.state != HotKeyState::Pressed {
            return;
        }
        if MainThreadMarker::new().is_none() {
            eprintln!("[rs] hotkey event arrived off the main thread; ignored");
            return;
        }
        if keytap::claim() {
            return; // the key tap already acted on this press
        }
        ui::event_log(if keytap::active() { "hotkey pressed (Carbon; the key tap missed it)" } else { "hotkey pressed" });
        toggle_panel();
    }));
    if registered {
        // Catches the same press earlier, ahead of other apps' keyboard taps.
        keytap::start(menu_modifiers(mods).bits() as u64, toggle_panel);
    }

    // Menu bar item: Open, Settings, Quit. It can be hidden from Settings.
    let shown_hotkey = if registered {
        Ok((name.clone(), menu_modifiers(mods)))
    } else {
        Err(hotkey_status.clone())
    };
    settings::install(&controller, shown_hotkey, mtm);
    app.setDelegate(Some(objc2::runtime::ProtocolObject::from_ref(&*controller)));

    // A rate that arrives while the panel is open is shown as a new presentation.
    rates::set_on_update(|| {
        let controller = CONTROLLER.with_borrow(|c| c.clone());
        if let Some(controller) = controller {
            controller.rates_updated();
        }
    });
    CONTROLLER.set(Some(controller));
    watch::start();
    app.run();
    drop(manager);
}
