//! Colour on the glass. A gradient layer sits inside the glass surface, under
//! the field and rows, so it moves with the opening spring and the resize
//! spring exactly as the glass does. Set by `theme` in config.toml:
//!
//!   "selection"  the selected result's colour: an app's icon, or a colour per
//!                kind of row; cross-fades as the selection moves (default)
//!   "aurora"     a slow drift through a few colours
//!   "accent"     the system accent colour
//!   "#7c5cff"    one fixed colour
//!   "none"       plain glass
//!
//! Under Reduce Motion colours change without fading and aurora stands still.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, msg_send};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSAutoresizingMaskOptions, NSBitmapImageRep,
    NSColor, NSCompositingOperation, NSGraphicsContext, NSImage, NSView, NSWorkspace,
};
use objc2_foundation::{NSArray, NSMutableArray, NSNumber, NSPoint, NSRect, NSSize, NSString};

#[derive(Clone, Debug, PartialEq)]
pub enum Theme {
    None,
    Selection,
    Aurora,
    Accent,
    Fixed(Rgb),
}

pub type Rgb = (f64, f64, f64);

/// `theme_strength`: "mild" (default), "medium", "strong", or 0 to 1.
pub fn strength(text: &str) -> f64 {
    match text.trim().to_lowercase().as_str() {
        "medium" => 0.75,
        "strong" => 1.0,
        "mild" => 0.5,
        other => other.parse::<f64>().map(|v| v.clamp(0.0, 1.0)).unwrap_or(0.5),
    }
}

pub fn parse(text: &str) -> Theme {
    let t = text.trim().to_lowercase();
    match t.as_str() {
        "none" | "off" | "plain" => Theme::None,
        "aurora" => Theme::Aurora,
        "accent" => Theme::Accent,
        _ => hex(&t).map(Theme::Fixed).unwrap_or(Theme::Selection),
    }
}

fn hex(t: &str) -> Option<Rgb> {
    let h = t.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some((f64::from((n >> 16) & 255) / 255.0, f64::from((n >> 8) & 255) / 255.0, f64::from(n & 255) / 255.0))
}

/// What the selected row is, for the "selection" theme.
pub enum Subject<'a> {
    App(&'a NSImage, &'a str),
    Calc,
    Units,
    Time,
    Link,
    Ask,
    Notice,
    Nothing,
}

fn kind_colour(subject: &Subject) -> Option<Rgb> {
    Some(match subject {
        Subject::Calc => (0.20, 0.78, 0.45),
        Subject::Units => (0.15, 0.70, 0.80),
        Subject::Time => (0.40, 0.38, 0.95),
        Subject::Link => (0.20, 0.52, 1.00),
        Subject::Ask => (0.72, 0.36, 0.98),
        Subject::Notice => (1.00, 0.72, 0.15),
        Subject::App(..) | Subject::Nothing => return None,
    })
}

// ---------------------------------------------------------------- colour maths

fn to_hsv((r, g, b): Rgb) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } * 60.0;
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

fn from_hsv(h: f64, s: f64, v: f64) -> Rgb {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r + m, g + m, b + m)
}

/// A colour bright and saturated enough to read through glass, and a
/// neighbour on the colour wheel for the gradient's far end.
fn pair(c: Rgb) -> (Rgb, Rgb) {
    let (h, s, v) = to_hsv(c);
    let (s, v) = (s.clamp(0.45, 0.9), v.clamp(0.75, 1.0));
    (from_hsv(h, s, v), from_hsv(h + 38.0, s, v))
}

/// The vivid colour of an app icon: drawn at 16×16, each pixel weighted by
/// opacity and saturation so a white or grey frame does not wash it out.
fn icon_colour(image: &NSImage) -> Option<Rgb> {
    const N: isize = 16;
    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            N,
            N,
            8,
            4,
            true,
            false,
            objc2_app_kit::NSDeviceRGBColorSpace,
            0,
            0,
        )?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        image.drawInRect_fromRect_operation_fraction(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(N as f64, N as f64)),
            NSRect::ZERO,
            NSCompositingOperation::Copy,
            1.0,
        );
        context.flushGraphics();
        NSGraphicsContext::restoreGraphicsState_class();
        let data = rep.bitmapData();
        if data.is_null() {
            return None;
        }
        let pixels = std::slice::from_raw_parts(data, (N * N * 4) as usize);
        let (mut sum, mut weight) = ((0.0, 0.0, 0.0), 0.0);
        let mut plain = ((0.0, 0.0, 0.0), 0.0);
        for p in pixels.chunks_exact(4) {
            let a = f64::from(p[3]) / 255.0;
            if a < 0.5 {
                continue;
            }
            // Premultiplied: undo it.
            let c = (f64::from(p[0]) / 255.0 / a, f64::from(p[1]) / 255.0 / a, f64::from(p[2]) / 255.0 / a);
            let (_, s, v) = to_hsv(c);
            let w = a * s * s * v;
            sum = (sum.0 + c.0 * w, sum.1 + c.1 * w, sum.2 + c.2 * w);
            weight += w;
            plain = ((plain.0.0 + c.0, plain.0.1 + c.1, plain.0.2 + c.2), plain.1 + 1.0);
        }
        if weight > 0.5 {
            Some((sum.0 / weight, sum.1 / weight, sum.2 / weight))
        } else if plain.1 > 0.0 {
            // A grey icon (Terminal, Settings): a quiet blue-grey, not nothing.
            Some((0.55, 0.62, 0.75))
        } else {
            None
        }
    }
}

fn accent() -> Rgb {
    let c = NSColor::controlAccentColor();
    c.colorUsingColorSpace(&objc2_app_kit::NSColorSpace::sRGBColorSpace())
        .map(|c| (c.redComponent(), c.greenComponent(), c.blueComponent()))
        .unwrap_or((0.0, 0.48, 1.0))
}

// ---------------------------------------------------------------- the layer

/// Stops of the gradient: (colour, opacity).
type Stops = [(Rgb, f64); 3];

pub struct Tint {
    view: Retained<NSView>,
    layer: Retained<AnyObject>,
    theme: RefCell<Theme>,
    /// `theme_strength`, 0 to 1.
    level: Cell<f64>,
    shown: RefCell<Option<Stops>>,
    icons: RefCell<HashMap<String, Option<Rgb>>>,
}

fn cg_colours(stops: &Stops) -> Retained<NSMutableArray<AnyObject>> {
    let array = NSMutableArray::<AnyObject>::new();
    for ((r, g, b), a) in stops {
        let colour = NSColor::colorWithSRGBRed_green_blue_alpha(*r, *g, *b, *a);
        let cg: *mut c_void = unsafe { msg_send![&*colour, CGColor] };
        // CGColor is toll-free bridged to an object, so the array retains it.
        unsafe { array.addObject(&*(cg as *const AnyObject)) };
    }
    array
}

fn reduce_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

fn dark(view: &NSView) -> bool {
    let names = NSArray::from_slice(&[unsafe { NSAppearanceNameAqua }, unsafe { NSAppearanceNameDarkAqua }]);
    let appearance: Retained<NSAppearance> = view.effectiveAppearance();
    appearance
        .bestMatchFromAppearancesWithNames(&names)
        .is_some_and(|n| &*n == unsafe { NSAppearanceNameDarkAqua })
}

impl Tint {
    /// A view the size of `rect` that fills its superview as it resizes.
    pub fn new(rect: NSRect, mtm: MainThreadMarker) -> Option<Tint> {
        let class = AnyClass::get(c"CAGradientLayer")?;
        let layer: Retained<AnyObject> = unsafe { msg_send![class, layer] };
        let view = NSView::initWithFrame(NSView::alloc(mtm), rect);
        // Layer-hosting: the view shows this layer and AppKit leaves it alone.
        unsafe {
            let _: () = msg_send![&*view, setLayer: &*layer];
            let _: () = msg_send![&*layer, setStartPoint: NSPoint::new(0.0, 1.0)];
            let _: () = msg_send![&*layer, setEndPoint: NSPoint::new(1.0, 0.0)];
            let locations = NSArray::from_retained_slice(&[NSNumber::new_f64(0.0), NSNumber::new_f64(0.55), NSNumber::new_f64(1.0)]);
            let _: () = msg_send![&*layer, setLocations: &*locations];
            // A hosted layer animates every change by itself, over 0.25 s. The
            // surface is resized on every frame of its spring, so the gradient
            // would trail the glass and shiver at its edge. Its size must
            // follow at once; colour fades are added explicitly below.
            let none: Retained<AnyObject> = msg_send![AnyClass::get(c"NSNull").unwrap(), null];
            let actions = objc2_foundation::NSMutableDictionary::<NSString, AnyObject>::new();
            for key in ["bounds", "position", "frame", "anchorPoint", "transform", "colors", "startPoint", "endPoint", "locations", "hidden", "opacity"] {
                actions.setObject_forKey(&*none, objc2::runtime::ProtocolObject::from_ref(&*NSString::from_str(key)));
            }
            let _: () = msg_send![&*layer, setActions: &*actions];
        }
        view.setWantsLayer(true);
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
        Some(Tint {
            view,
            layer,
            theme: RefCell::new(Theme::Selection),
            level: Cell::new(0.5),
            shown: RefCell::new(None),
            icons: RefCell::new(HashMap::new()),
        })
    }

    pub fn view(&self) -> &NSView {
        &self.view
    }

    pub fn set_theme(&self, theme: Theme, level: f64) {
        if *self.theme.borrow() != theme || self.level.get() != level {
            *self.theme.borrow_mut() = theme;
            self.level.set(level);
            self.stop();
        }
    }

    /// Stronger on dark glass, where colour reads weaker.
    /// At full strength; stronger on dark glass, where colour reads weaker.
    fn strength(&self) -> f64 {
        self.level.get() * if dark(&self.view) { 0.34 } else { 0.24 }
    }

    fn stops_for(&self, main: Rgb, soft: f64) -> Stops {
        let (a, b) = pair(main);
        let k = self.strength() * soft;
        [(a, k), (b, k * 0.55), (b, k * 0.18)]
    }

    /// Show the colours for `subject` now, cross-fading from what is shown.
    pub fn update(&self, subject: &Subject) {
        let theme = self.theme.borrow().clone();
        let main = match theme {
            Theme::None => None,
            Theme::Accent => Some(accent()),
            Theme::Fixed(c) => Some(c),
            Theme::Aurora => return self.drift(),
            Theme::Selection => match subject {
                Subject::App(image, path) => {
                    let known = self.icons.borrow().get(*path).copied();
                    known.unwrap_or_else(|| {
                        let c = icon_colour(image);
                        self.icons.borrow_mut().insert(path.to_string(), c);
                        c
                    })
                }
                // The empty field keeps a faint accent colour (see `soft`).
                Subject::Nothing => Some(accent()),
                other => kind_colour(other),
            },
        };
        // Nothing selected: the colour stands for nothing, so it is fainter.
        let soft = if matches!((&theme, subject), (Theme::Selection, Subject::Nothing)) { 0.55 } else { 1.0 };
        let stops = match main {
            Some(c) => self.stops_for(c, soft),
            None => [((0.0, 0.0, 0.0), 0.0); 3],
        };
        if self.shown.borrow().as_ref() == Some(&stops) {
            return;
        }
        let new = cg_colours(&stops);
        unsafe {
            let old: Option<Retained<AnyObject>> = msg_send![&*self.layer, colors];
            if let (Some(old), false, Some(class)) = (old, reduce_motion(), AnyClass::get(c"CABasicAnimation")) {
                let fade: Retained<AnyObject> = msg_send![class, animationWithKeyPath: &*NSString::from_str("colors")];
                let _: () = msg_send![&*fade, setFromValue: &*old];
                let _: () = msg_send![&*fade, setToValue: &*new];
                let _: () = msg_send![&*fade, setDuration: 0.32f64];
                let _: () = msg_send![&*self.layer, addAnimation: &*fade, forKey: &*NSString::from_str("rs.tint")];
            }
            let _: () = msg_send![&*self.layer, setColors: &*new];
        }
        *self.shown.borrow_mut() = Some(stops);
    }

    /// Aurora: four palettes, about 20 s round, and the light direction turning.
    fn drift(&self) {
        let k = self.strength();
        let palettes: [[Rgb; 3]; 4] = [
            [(0.35, 0.40, 1.00), (0.75, 0.35, 0.95), (0.10, 0.75, 0.85)],
            [(0.85, 0.30, 0.70), (1.00, 0.55, 0.30), (0.45, 0.35, 1.00)],
            [(0.15, 0.75, 0.70), (0.30, 0.55, 1.00), (0.80, 0.40, 0.90)],
            [(1.00, 0.60, 0.25), (0.90, 0.30, 0.55), (0.25, 0.70, 0.95)],
        ];
        let to_stops = |p: &[Rgb; 3]| -> Stops { [(p[0], k), (p[1], k * 0.6), (p[2], k * 0.35)] };
        let first = to_stops(&palettes[0]);
        if self.shown.borrow().as_ref() == Some(&first) {
            return; // already drifting
        }
        unsafe {
            let _: () = msg_send![&*self.layer, setColors: &*cg_colours(&first)];
        }
        *self.shown.borrow_mut() = Some(first);
        if reduce_motion() {
            return;
        }
        let Some(class) = AnyClass::get(c"CAKeyframeAnimation") else { return };
        unsafe {
            let values = NSMutableArray::<AnyObject>::new();
            for p in palettes.iter().chain(std::iter::once(&palettes[0])) {
                values.addObject(&*cg_colours(&to_stops(p)));
            }
            let colours: Retained<AnyObject> = msg_send![class, animationWithKeyPath: &*NSString::from_str("colors")];
            let _: () = msg_send![&*colours, setValues: &*values];
            let _: () = msg_send![&*colours, setDuration: 20.0f64];
            let _: () = msg_send![&*colours, setRepeatCount: f32::INFINITY];
            let _: () = msg_send![&*self.layer, addAnimation: &*colours, forKey: &*NSString::from_str("rs.aurora")];

            for (key, points) in [
                ("startPoint", [(0.0, 1.0), (0.5, 1.0), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0)]),
                ("endPoint", [(1.0, 0.0), (0.5, 0.0), (0.0, 0.0), (0.5, 0.0), (1.0, 0.0)]),
            ] {
                let values = NSMutableArray::<AnyObject>::new();
                for (x, y) in points {
                    let v: Retained<AnyObject> = msg_send![AnyClass::get(c"NSValue").unwrap(), valueWithPoint: NSPoint::new(x, y)];
                    values.addObject(&*v);
                }
                let turn: Retained<AnyObject> = msg_send![class, animationWithKeyPath: &*NSString::from_str(key)];
                let _: () = msg_send![&*turn, setValues: &*values];
                let _: () = msg_send![&*turn, setDuration: 26.0f64];
                let _: () = msg_send![&*turn, setRepeatCount: f32::INFINITY];
                let _: () = msg_send![&*self.layer, addAnimation: &*turn, forKey: &*NSString::from_str(&format!("rs.aurora.{key}"))];
            }
        }
    }

    /// The panel was hidden: stop any drift, and start from scratch next time.
    pub fn stop(&self) {
        unsafe {
            let _: () = msg_send![&*self.layer, removeAllAnimations];
        }
        *self.shown.borrow_mut() = None;
    }
}
