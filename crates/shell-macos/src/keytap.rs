//! Catches the global shortcut before other apps' keyboard tools can hold or
//! swallow it.
//!
//! The Carbon hotkey is matched only after every other app's event tap has
//! passed the key on, so a tap that stalls (Wispr Flow, BetterTouchTool were
//! both seen holding keys for seconds) can delay or lose Command-Space. This
//! adds a listen-only tap at the hardware end of the chain, ahead of theirs.
//! It needs Input Monitoring. It only watches: the Carbon hotkey still
//! consumes the key, and still works alone when the tap is off (no
//! permission, or secure keyboard input, which hides keys from all taps).
//!
//! Each press the tap acts on is remembered for a few seconds, so the Carbon
//! event for the same press is ignored rather than toggling the panel back.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::ui::event_log;

type Port = *mut c_void;
type Callback = extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, events: u64, callback: Callback, user: *mut c_void) -> Port;
    fn CGEventTapEnable(tap: Port, enable: bool);
    fn CGEventGetIntegerValueField(event: *mut c_void, field: u32) -> i64;
    fn CGEventGetFlags(event: *mut c_void) -> u64;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFMachPortCreateRunLoopSource(alloc: *const c_void, port: Port, order: isize) -> *mut c_void;
    fn CFRunLoopGetMain() -> *mut c_void;
    fn CFRunLoopAddSource(run_loop: *mut c_void, source: *mut c_void, mode: *const c_void);
    static kCFRunLoopCommonModes: *const c_void;
}

const HID_EVENT_TAP: u32 = 0;
const HEAD_INSERT: u32 = 0;
const LISTEN_ONLY: u32 = 1;
const KEY_DOWN: u32 = 10;
const DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
const FIELD_AUTOREPEAT: u32 = 8;
const FIELD_KEYCODE: u32 = 9;
const SPACE: i64 = 49;
/// Shift, Control, Option, Command: the same bits as NSEventModifierFlags.
const MODIFIERS: u64 = 0x1E_0000;
/// Longest a stalled tap was seen to hold a key, with room to spare.
const SAME_PRESS: Duration = Duration::from_secs(5);

thread_local! {
    static TAP: Cell<Port> = const { Cell::new(std::ptr::null_mut()) };
    static WANT: Cell<u64> = const { Cell::new(0) };
    static ON_PRESS: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    /// Presses the tap acted on that the Carbon event has not yet matched.
    static PENDING: RefCell<VecDeque<Instant>> = const { RefCell::new(VecDeque::new()) };
    static WARNED: Cell<bool> = const { Cell::new(false) };
}

extern "C" fn on_event(_proxy: *mut c_void, kind: u32, event: *mut c_void, _user: *mut c_void) -> *mut c_void {
    match kind {
        DISABLED_BY_TIMEOUT | DISABLED_BY_USER_INPUT => {
            event_log("key tap was switched off by the system; switched back on");
            let tap = TAP.get();
            if !tap.is_null() {
                unsafe { CGEventTapEnable(tap, true) };
            }
        }
        KEY_DOWN => unsafe {
            let is_space = CGEventGetIntegerValueField(event, FIELD_KEYCODE) == SPACE;
            if is_space
                && CGEventGetIntegerValueField(event, FIELD_AUTOREPEAT) == 0
                && CGEventGetFlags(event) & MODIFIERS == WANT.get()
            {
                pressed();
            }
        },
        _ => {}
    }
    event
}

fn pressed() {
    PENDING.with_borrow_mut(|p| p.push_back(Instant::now()));
    event_log("hotkey pressed (key tap)");
    let on_press = ON_PRESS.with_borrow(|f| f.clone());
    if let Some(f) = on_press {
        f();
    }
}

fn create() -> bool {
    let tap = unsafe { CGEventTapCreate(HID_EVENT_TAP, HEAD_INSERT, LISTEN_ONLY, 1 << KEY_DOWN, on_event, std::ptr::null_mut()) };
    if tap.is_null() {
        return false;
    }
    unsafe {
        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        CFRunLoopAddSource(CFRunLoopGetMain(), source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
    }
    TAP.set(tap);
    event_log("key tap on: the shortcut is caught ahead of other apps' keyboard tools");
    crate::settings::refresh();
    true
}

/// Start watching for `modifiers` + Space. Asks for Input Monitoring the
/// first time; until it is allowed, the Carbon hotkey works alone.
pub fn start(modifiers: u64, on_press: impl Fn() + 'static) {
    WANT.set(modifiers & MODIFIERS);
    ON_PRESS.set(Some(Rc::new(on_press)));
    if unsafe { CGPreflightListenEventAccess() } {
        if !create() {
            WARNED.set(true);
            event_log("key tap could not be created although Input Monitoring is allowed");
        }
    } else {
        event_log("key tap off: Input Monitoring is not allowed yet");
        // Shows the system prompt once; afterwards it only answers.
        unsafe { CGRequestListenEventAccess() };
    }
}

/// Called every couple of seconds: picks up a permission granted since
/// launch, and logs a press the Carbon hotkey never delivered.
pub fn tick() {
    if WANT.get() != 0 && TAP.get().is_null() && unsafe { CGPreflightListenEventAccess() } && !create() && !WARNED.replace(true) {
        event_log("key tap could not be created although Input Monitoring is allowed");
    }
    let lost = PENDING.with_borrow_mut(|p| {
        let before = p.len();
        p.retain(|t| t.elapsed() < SAME_PRESS);
        before - p.len()
    });
    for _ in 0..lost {
        event_log("the Carbon hotkey never arrived for that press; the key tap caught it");
    }
}

/// The Carbon hotkey fired. True if the tap already acted on this press, in
/// which case the event is ignored.
pub fn claim() -> bool {
    PENDING.with_borrow_mut(|p| {
        p.retain(|t| t.elapsed() < SAME_PRESS);
        p.pop_front().is_some()
    })
}

pub fn active() -> bool {
    !TAP.get().is_null()
}

/// Open System Settings at Privacy & Security > Input Monitoring.
pub fn open_settings() {
    let _ = std::process::Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
        .spawn();
}
