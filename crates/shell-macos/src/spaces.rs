//! Which Spaces the window server keeps a window in.
//!
//! A long-running panel was found listed in the desktop Space only (a
//! healthy one is in no Space before its first showing and in all of them
//! after), and from then on the window server showed it only there: over a
//! full-screen app it was "shown" but off screen, although AppKit and the
//! window's own "all Spaces" flag both said otherwise. AppKit offers no way to see or undo that, so this asks the
//! window server directly, through the private calls window managers such as
//! yabai use. They are looked up at run time; if one is missing, nothing here
//! does anything.

use std::ffi::{CStr, c_void};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSArray, NSNumber};

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
}

/// `RTLD_DEFAULT` on macOS: search every loaded image.
const DEFAULT: *mut c_void = -2isize as *mut c_void;
/// Current, other and user Spaces.
const ALL_SPACES: i32 = 7;

type MainConnection = unsafe extern "C" fn() -> i32;
type ActiveSpace = unsafe extern "C" fn(i32) -> u64;
type CopySpaces = unsafe extern "C" fn(i32, i32, *const AnyObject) -> *mut AnyObject;
type ChangeSpaces = unsafe extern "C" fn(i32, *const AnyObject, *const AnyObject);

fn symbol<T: Copy>(name: &CStr) -> Option<T> {
    let p = unsafe { dlsym(DEFAULT, name.as_ptr()) };
    // SAFETY: T is always an `extern "C" fn` type matching the symbol.
    (!p.is_null()).then(|| unsafe { std::mem::transmute_copy::<*mut c_void, T>(&p) })
}

fn connection() -> Option<i32> {
    symbol::<MainConnection>(c"CGSMainConnectionID").map(|f| unsafe { f() })
}

fn numbers(values: &[i64]) -> Retained<NSArray<NSNumber>> {
    let items: Vec<Retained<NSNumber>> = values.iter().map(|&v| NSNumber::new_i64(v)).collect();
    NSArray::from_retained_slice(&items)
}

/// The Space the user is looking at, on the main display.
pub fn active() -> Option<u64> {
    let cid = connection()?;
    symbol::<ActiveSpace>(c"CGSGetActiveSpace").map(|f| unsafe { f(cid) })
}

/// The Spaces the window server holds `window` in.
pub fn of_window(window: isize) -> Option<Vec<u64>> {
    let cid = connection()?;
    let copy = symbol::<CopySpaces>(c"CGSCopySpacesForWindows")?;
    let windows = numbers(&[window as i64]);
    let list = unsafe { copy(cid, ALL_SPACES, Retained::as_ptr(&windows).cast()) };
    if list.is_null() {
        return None;
    }
    // A CFArray of CFNumbers, toll-free bridged; returned retained.
    let list: Retained<NSArray<NSNumber>> = unsafe { Retained::from_raw(list.cast())? };
    Some(list.iter().map(|n| n.unsignedLongLongValue()).collect())
}

fn change(name: &CStr, window: isize, spaces: &[u64]) {
    let (Some(cid), Some(f)) = (connection(), symbol::<ChangeSpaces>(name)) else { return };
    let windows = numbers(&[window as i64]);
    let spaces = numbers(&spaces.iter().map(|&s| s as i64).collect::<Vec<_>>());
    unsafe { f(cid, Retained::as_ptr(&windows).cast(), Retained::as_ptr(&spaces).cast()) };
}

pub fn add(window: isize, space: u64) {
    change(c"CGSAddWindowsToSpaces", window, &[space]);
}
