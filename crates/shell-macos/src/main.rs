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
mod hotkey;
mod keytap;
mod rates;
mod settings;
mod store;
mod theme;
mod ui;
mod watch;

use std::cell::RefCell;
use std::time::Instant;

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

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
    hotkey::init(&wanted, toggle_panel);

    // Menu bar item: Open, Settings, Quit. It can be hidden from Settings.
    settings::install(&controller, mtm);
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
}
