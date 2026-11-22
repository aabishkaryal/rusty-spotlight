//! Watches for the two things that can keep a global shortcut from arriving:
//! secure keyboard input (an app with a password field focused) and another
//! app's keyboard event tap that stalls or gets switched off. Changes are
//! written to the log, so a missed Command-Space can be matched to a cause.
//! Diagnostic only: it never changes what the launcher does.

use std::cell::RefCell;
use std::collections::HashMap;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::msg_send;
use objc2_foundation::{NSNumber, NSString, NSTimer};

use crate::ui::event_log;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct TapInfo {
    tap_id: u32,
    tap_point: u32,
    options: u32,
    events_of_interest: u64,
    tapping_process: i32,
    process_being_tapped: i32,
    enabled: bool,
    min_usec_latency: f32,
    avg_usec_latency: f32,
    max_usec_latency: f32,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGGetEventTapList(max: u32, list: *mut TapInfo, count: *mut u32) -> i32;
    fn CGSessionCopyCurrentDictionary() -> *mut AnyObject;
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn IsSecureEventInputEnabled() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(object: *mut AnyObject);
}

const KEY_EVENTS: u64 = (1 << 10) | (1 << 11) | (1 << 12); // keyDown, keyUp, flagsChanged
/// A tap this slow holds every keystroke, Command-Space included.
const SLOW_USEC: f32 = 250_000.0;

/// The system reports a tap's worst delay since it was created, so a new
/// record is what marks a fresh stall. `worst_usec` is the last one logged.
#[derive(Clone, Copy)]
struct TapState {
    enabled: bool,
    worst_usec: f32,
}

#[derive(Default)]
struct Seen {
    secure: Option<bool>,
    taps: HashMap<(i32, u32), TapState>,
}

thread_local! {
    static SEEN: RefCell<Seen> = RefCell::new(Seen::default());
    static TIMER: RefCell<Option<Retained<NSTimer>>> = const { RefCell::new(None) };
}

fn process_name(pid: i32) -> String {
    std::process::Command::new("/bin/ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().rsplit('/').next().unwrap_or("").to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("pid {pid}"))
}

fn secure_input_pid() -> Option<i32> {
    unsafe {
        let dict = CGSessionCopyCurrentDictionary();
        if dict.is_null() {
            return None;
        }
        let key = NSString::from_str("kCGSSessionSecureInputPID");
        let value: *mut AnyObject = msg_send![dict, objectForKey: &*key];
        let pid = (!value.is_null()).then(|| {
            let n: &NSNumber = &*(value as *const NSNumber);
            n.intValue()
        });
        CFRelease(dict);
        pid
    }
}

fn check() {
    crate::keytap::tick();
    let me = std::process::id() as i32;
    let secure = unsafe { IsSecureEventInputEnabled() };
    let mut n = 0u32;
    let mut taps = Vec::new();
    unsafe {
        if CGGetEventTapList(0, std::ptr::null_mut(), &mut n) == 0 && n > 0 {
            taps.resize(n as usize, TapInfo::default());
            if CGGetEventTapList(n, taps.as_mut_ptr(), &mut n) != 0 {
                n = 0;
            }
            taps.truncate(n as usize);
        }
    }
    SEEN.with_borrow_mut(|seen| {
        if seen.secure != Some(secure) {
            if secure || seen.secure.is_some() {
                let who = secure_input_pid().map(process_name).unwrap_or_default();
                event_log(&if secure {
                    format!("secure keyboard input ON ({who}): global shortcuts may not arrive")
                } else {
                    "secure keyboard input off".to_string()
                });
            }
            seen.secure = Some(secure);
        }
        let mut current = HashMap::new();
        for t in &taps {
            // Only filtering (not listen-only) taps on keys can hold or drop them.
            if t.tapping_process == me || t.events_of_interest & KEY_EVENTS == 0 || t.options == 1 {
                continue;
            }
            let key = (t.tapping_process, t.tap_id);
            let before = seen.taps.get(&key).copied();
            // A tap its app stopped serving reports a worst delay that grows
            // with the clock: log it again only when it has doubled.
            let logged = before.map_or(0.0, |b| b.worst_usec);
            let stalled = t.enabled && t.max_usec_latency > SLOW_USEC && t.max_usec_latency > 2.0 * logged;
            let report = match before {
                Some(b) => b.enabled != t.enabled || stalled,
                None => !t.enabled || stalled,
            };
            let state = TapState { enabled: t.enabled, worst_usec: if report { t.max_usec_latency } else { logged } };
            if report {
                event_log(&format!(
                    "keyboard tap of {}: {}, worst delay {:.0} ms",
                    process_name(t.tapping_process),
                    if t.enabled { "on" } else { "OFF (timed out or stopped)" },
                    t.max_usec_latency / 1000.0
                ));
            }
            current.insert(key, state);
        }
        seen.taps = current;
    });
}

/// Check every two seconds for the life of the app.
pub fn start() {
    check();
    let block = block2::RcBlock::new(|_: std::ptr::NonNull<NSTimer>| check());
    let timer = unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(2.0, true, &block) };
    TIMER.set(Some(timer));
}
