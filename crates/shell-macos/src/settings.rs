//! The menu bar item and the Settings window.
//!
//! The icon can be hidden (`menu_bar_icon = false`). Settings then stay
//! reachable from the launcher (type "settings") and by opening the app again
//! in Finder, which macOS delivers as a reopen request.

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBox, NSBoxType, NSButton, NSColor, NSControlStateValueOff,
    NSControlStateValueOn, NSEvent, NSEventModifierFlags, NSFont, NSFontWeightMedium, NSImage,
    NSImageSymbolConfiguration, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSTextField,
    NSVariableStatusItemLength, NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::config;
use crate::ui::Controller;

thread_local! {
    static STATUS: RefCell<Option<Retained<NSStatusItem>>> = const { RefCell::new(None) };
    static ICON_WANTED: Cell<bool> = const { Cell::new(true) };
    static WINDOW: RefCell<Option<Retained<SettingsWindow>>> = const { RefCell::new(None) };
    static CHECKBOX: RefCell<Option<Retained<NSButton>>> = const { RefCell::new(None) };
    /// "Command-Space", or why the shortcut could not be registered.
    static HOTKEY: RefCell<(String, bool)> = const { RefCell::new((String::new(), false)) };
    static TAP_NOTE: RefCell<Option<(Retained<NSTextField>, Retained<NSButton>)>> = const { RefCell::new(None) };
}

const TAP_ON: &str = "Caught ahead of other apps’ keyboard tools, so they can’t delay or swallow it.";
const TAP_OFF: &str = "Allow Rusty Spotlight in Input Monitoring so other apps’ keyboard tools can’t delay or swallow the shortcut.";

/// Show whether the key tap is on. Called when it starts.
pub fn refresh() {
    TAP_NOTE.with_borrow(|t| {
        if let Some((label, button)) = t {
            let on = crate::keytap::active();
            label.setStringValue(&ns(if on { TAP_ON } else { TAP_OFF }));
            button.setHidden(on);
        }
    });
}

fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

// ---------------------------------------------------------------- menu bar

/// Build the menu bar item. `hotkey` is the shortcut's name and the modifiers
/// to draw beside "Open", or `None` with the reason it is unavailable.
pub fn install(controller: &Controller, hotkey: Result<(String, NSEventModifierFlags), String>, mtm: MainThreadMarker) {
    let status = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = status.button(mtm) {
        // The system magnifying glass, drawn as a template so it follows the
        // menu bar's appearance like the icons beside it.
        let symbol = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns("magnifyingglass"), Some(&ns("Rusty Spotlight")));
        let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(14.0, unsafe { NSFontWeightMedium });
        if let Some(image) = symbol.and_then(|s| s.imageWithSymbolConfiguration(&config)) {
            image.setTemplate(true);
            button.setImage(Some(&image));
            button.setTitle(&ns(""));
        } else {
            button.setTitle(&ns("⌕"));
        }
    }

    let menu = NSMenu::new(mtm);
    let item = |title: &str, action: Option<Sel>, key: &str| unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(NSMenuItem::alloc(mtm), &ns(title), action, &ns(key))
    };
    let target: &AnyObject = controller;

    let open = item("Open Rusty Spotlight", Some(sel!(showPanel:)), "");
    unsafe { open.setTarget(Some(target)) };
    match &hotkey {
        Ok((name, mods)) => {
            // Drawn only, as a reminder: the global shortcut does the work.
            open.setKeyEquivalent(&ns(" "));
            open.setKeyEquivalentModifierMask(*mods);
            HOTKEY.set((name.clone(), true));
        }
        Err(reason) => HOTKEY.set((reason.clone(), false)),
    }
    menu.addItem(&open);
    if let Err(reason) = &hotkey {
        let note = item(reason, None, "");
        note.setEnabled(false);
        menu.addItem(&note);
    }
    let settings = item("Settings…", Some(sel!(showSettings:)), ",");
    unsafe { settings.setTarget(Some(target)) };
    menu.addItem(&settings);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&item("Quit Rusty Spotlight", Some(sel!(terminate:)), "q"));
    status.setMenu(Some(&menu));

    status.setVisible(ICON_WANTED.get());
    STATUS.set(Some(status));
}

/// Show or hide the icon. Called whenever the configuration is read.
pub fn set_icon_visible(visible: bool) {
    if ICON_WANTED.replace(visible) != visible {
        crate::ui::event_log(if visible { "menu bar icon shown" } else { "menu bar icon hidden" });
    }
    STATUS.with_borrow(|s| {
        if let Some(s) = s {
            s.setVisible(visible);
        }
    });
    CHECKBOX.with_borrow(|c| {
        if let Some(c) = c {
            c.setState(if visible { NSControlStateValueOn } else { NSControlStateValueOff });
        }
    });
}

// ---------------------------------------------------------------- window

define_class!(
    #[unsafe(super(NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[name = "RSSettingsWindow"]
    pub struct SettingsWindow;

    impl SettingsWindow {
        // An agent app has no menu bar, so Command-W and Escape are handled here.
        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            self.close_shortcut(event) || unsafe { msg_send![super(self), performKeyEquivalent: event] }
        }

        #[unsafe(method(cancelOperation:))]
        fn cancel_operation(&self, _sender: Option<&AnyObject>) {
            self.close();
        }
    }
);

impl SettingsWindow {
    fn close_shortcut(&self, event: &NSEvent) -> bool {
        let flags = event.modifierFlags() & NSEventModifierFlags::DeviceIndependentFlagsMask;
        let key = event.charactersIgnoringModifiers().map(|c| c.to_string());
        let close = flags == NSEventModifierFlags::Command && key.as_deref() == Some("w");
        if close {
            self.close();
        }
        close
    }
}

const WIDTH: f64 = 440.0;
const PAD: f64 = 20.0;

fn label(text: &str, size: f64, secondary: bool, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let l = NSTextField::wrappingLabelWithString(&ns(text), mtm);
    l.setFont(Some(&NSFont::systemFontOfSize(size)));
    if secondary {
        l.setTextColor(Some(&NSColor::secondaryLabelColor()));
    }
    l.setSelectable(false);
    l
}

/// Place `view` at `y` (from the top), `height` tall, indented by `indent`.
fn put(content: &NSView, view: &NSView, top: &mut f64, height: f64, indent: f64, total: f64) {
    view.setFrame(NSRect::new(NSPoint::new(PAD + indent, total - *top - height), NSSize::new(WIDTH - 2.0 * PAD - indent, height)));
    content.addSubview(view);
    *top += height;
}

fn build(controller: &Controller, mtm: MainThreadMarker) -> Retained<SettingsWindow> {
    const HEIGHT: f64 = 262.0;
    let window: Retained<SettingsWindow> = unsafe {
        msg_send![
            SettingsWindow::alloc(mtm),
            initWithContentRect: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT)),
            styleMask: NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
            backing: NSBackingStoreType::Buffered,
            defer: false,
        ]
    };
    unsafe { window.setReleasedWhenClosed(false) };
    window.setTitle(&ns("Rusty Spotlight Settings"));
    // Open on the Space in use, even over a full-screen app, rather than
    // switching to another desktop.
    window.setCollectionBehavior(NSWindowCollectionBehavior::MoveToActiveSpace | NSWindowCollectionBehavior::FullScreenAuxiliary);
    let content = window.contentView().expect("a window has a content view");
    let target: &AnyObject = controller;
    let mut top = PAD;

    // Menu bar icon.
    let checkbox = unsafe {
        NSButton::checkboxWithTitle_target_action(&ns("Show icon in menu bar"), Some(target), Some(sel!(toggleMenuBarIcon:)), mtm)
    };
    checkbox.setFont(Some(&NSFont::systemFontOfSize_weight(13.0, unsafe { NSFontWeightMedium })));
    checkbox.setState(if ICON_WANTED.get() { NSControlStateValueOn } else { NSControlStateValueOff });
    put(&content, &checkbox, &mut top, 20.0, 0.0, HEIGHT);
    top += 4.0;
    let hint = label(
        "When the icon is hidden, type “settings” in Rusty Spotlight, or open the app again from Finder, to come back here.",
        11.0,
        true,
        mtm,
    );
    put(&content, &hint, &mut top, 30.0, 20.0, HEIGHT);
    CHECKBOX.set(Some(checkbox));

    top += 14.0;
    let line = NSBox::new(mtm);
    line.setBoxType(NSBoxType::Separator);
    put(&content, &line, &mut top, 1.0, 0.0, HEIGHT);
    top += 14.0;

    // Shortcut, read-only here: it is set in the configuration file.
    let (hotkey, ok) = HOTKEY.with_borrow(|h| h.clone());
    let title = if ok { format!("Shortcut: {hotkey}") } else { hotkey };
    let shortcut = label(&title, 13.0, false, mtm);
    shortcut.setFont(Some(&NSFont::systemFontOfSize_weight(13.0, unsafe { NSFontWeightMedium })));
    put(&content, &shortcut, &mut top, 18.0, 0.0, HEIGHT);
    top += 2.0;
    let tap_note = label(TAP_OFF, 11.0, true, mtm);
    put(&content, &tap_note, &mut top, 30.0, 0.0, HEIGHT);
    top += 8.0;
    let note = label(
        "Change “hotkey” in the configuration file, then restart Rusty Spotlight. Links, aliases and Ask tools are set there too.",
        11.0,
        true,
        mtm,
    );
    put(&content, &note, &mut top, 30.0, 0.0, HEIGHT);
    top += 12.0;

    let open = unsafe {
        NSButton::buttonWithTitle_target_action(&ns("Open Configuration File"), Some(target), Some(sel!(openConfigFile:)), mtm)
    };
    open.sizeToFit();
    let width = open.frame().size.width;
    open.setFrame(NSRect::new(NSPoint::new(PAD - 6.0, HEIGHT - top - 32.0), NSSize::new(width, 32.0)));
    content.addSubview(&open);
    let allow = unsafe {
        NSButton::buttonWithTitle_target_action(&ns("Open Input Monitoring…"), Some(target), Some(sel!(openInputMonitoring:)), mtm)
    };
    allow.sizeToFit();
    let allow_width = allow.frame().size.width;
    allow.setFrame(NSRect::new(NSPoint::new(PAD - 6.0 + width + 4.0, HEIGHT - top - 32.0), NSSize::new(allow_width, 32.0)));
    content.addSubview(&allow);
    TAP_NOTE.set(Some((tap_note, allow)));
    refresh();

    window.center();
    window
}

/// Bring the Settings window forward, building it on first use.
pub fn show(controller: &Controller) {
    let mtm = controller.mtm();
    let window = WINDOW.with_borrow(|w| w.clone()).unwrap_or_else(|| {
        let w = build(controller, mtm);
        WINDOW.set(Some(w.clone()));
        w
    });
    // An agent app has to be made active for its window to take the keyboard.
    refresh();
    NSApplication::sharedApplication(mtm).activate();
    window.makeKeyAndOrderFront(None);
}

/// The checkbox was clicked: save the choice and apply it now.
pub fn toggled(sender: Option<&AnyObject>) {
    let on: isize = sender.map(|s| unsafe { msg_send![s, state] }).unwrap_or(1);
    let visible = on != 0;
    if let Err(e) = config::set_value("menu_bar_icon", if visible { "true" } else { "false" }) {
        crate::ui::event_log(&format!("could not save menu_bar_icon: {e}"));
    }
    set_icon_visible(visible);
}

// ---------------------------------------------------------------- snapshot

/// `rusty-spotlight --snapshot-settings DIR`: draw the Settings window's
/// contents and the menu bar icon to PNG files without showing anything, so
/// the layout can be checked from a script.
pub fn snapshot(controller: &Controller, dir: &str) {
    use objc2_app_kit::{NSBitmapImageFileType, NSImageView};
    use objc2_foundation::NSDictionary;
    let mtm = controller.mtm();
    let save = |view: &NSView, name: &str| {
        let bounds = view.bounds();
        if let Some(rep) = view.bitmapImageRepForCachingDisplayInRect(bounds) {
            view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
            let data = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) };
            if let Some(data) = data {
                let _ = std::fs::write(format!("{dir}/{name}"), data.to_vec());
            }
        }
    };
    if HOTKEY.with_borrow(|h| h.0.is_empty()) {
        HOTKEY.set(("Command-Space".into(), true));
    }
    let window = build(controller, mtm);
    if let Some(content) = window.contentView() {
        save(&content, "settings.png");
    }
    // The icon as the menu bar draws it: a template tinted with the label colour.
    let symbol = NSImage::imageWithSystemSymbolName_accessibilityDescription(&ns("magnifyingglass"), None);
    let config = NSImageSymbolConfiguration::configurationWithPointSize_weight(14.0, unsafe { NSFontWeightMedium });
    if let Some(image) = symbol.and_then(|s| s.imageWithSymbolConfiguration(&config)) {
        image.setTemplate(true);
        let host = NSView::initWithFrame(NSView::alloc(mtm), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(40.0, 24.0)));
        host.setWantsLayer(true);
        let view = NSImageView::imageViewWithImage(&image, mtm);
        view.setContentTintColor(Some(&NSColor::labelColor()));
        view.setFrame(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(40.0, 24.0)));
        host.addSubview(&view);
        save(&host, "icon.png");
    }
}
