# Council round 1 — Grok

Date of research: 2026-11-21. Author role: one of three independent advisors. This is a recommendation, not a survey. The picks are at the top. The rest is the evidence and the plan.

The developer is on macOS 27, Apple Silicon, Rust 1.100. The tree at `/Users/spydo/side-projects/rusty-spotlight` had no code when this was written. No Cargo project was created.

## How claims are marked

- **[searched 2026-11-21]** — a page, crate listing, release note, or repo README opened or returned by search today. Versions and dates below come from those pages.
- **[secondary]** — a write-up of someone else's primary source. Used when the primary page did not come back in full. Named as such.
- **[platform]** — long-standing macOS behavior (AppKit, Launch Services, TCC, pasteboard). Not re-derived from an Apple doc fetch today. The two-week spikes exist to confirm the parts that have drifted.
- **[memory]** — something I did not re-check today. Rare, and labeled in place.

A low-trust page is called out rather than averaged in. The johal.in "Raycast 2.0 vs Alfred 5.0" benchmark (2026-05-04, <https://johal.in/benchmark-raycast-20-vs-alfred-50-launch-speed>) gives idle RAM of 120 MB for Raycast and 85 MB for Alfred, and cold starts under a second. That contradicts both Raycast's own May 2026 figures and Asyar's October 2026 measurement. Ignore it.

---

## Verdict

Build a resident menu-bar agent whose window is an AppKit `NSPanel`, written in Rust through `objc2`, with the search field as a real `NSTextField`. Keep the match engine, frecency, and providers in a pure Rust crate. Ship scripts (a process, JSON on stdout) as the only extension mechanism in v1. Query Spotlight for files. Do not build a file index, a React runtime, a Raycast-compatible API, a WASM host, or a cross-platform UI in the first four months.

The product earns the word "fast" by being the thing Raycast's own engineers measured as cheap: the native shell, which they put at about 40 MB, while the WebView and the Node backend cost 120–200 MB each ([secondary], Raycast's 2026-05-14 deep dive, figures quoted consistently by <https://www.merlin.computer/blog/raycast-2-memory-usage-mac-2026> and <https://www.raycast.com/blog/a-technical-deep-dive-into-the-new-raycast>). Asyar, the serious Rust+Tauri launcher, measured itself at 419 MB idle against Raycast 2.6 at 452 MB, on an M4 Max running macOS 27.0.1, on 2026-10-04 ([searched], <https://github.com/Xoshbin/asyar>). Tauri can win the hotkey stopwatch and still lose the reason to exist.

Targets, on Apple Silicon, release build, window hidden unless noted:

| Budget | Target | What it beats |
| --- | --- | --- |
| Hotkey → panel pixels, p50 / p95 | < 16 ms / < 33 ms | Asyar measured Raycast at 22.2 / 32.0 ms; Asyar itself at 13.9 / 17.3 ms. Match Asyar's p50, beat Raycast's p95. |
| Hotkey → can type, with recents already filled, p95 | < 50 ms | Resident process. No WebView boot. |
| Keystroke → apps/commands painted, ≤ 20k items, p95 | < 8 ms | Matcher time here is noise. The budget is the UI thread. |
| Keystroke → first file row (Spotlight), p95 | < 80 ms, then stream, cancel on the next key | Do not block the app list on `mdfind`. |
| Idle RSS, all processes, window hidden, no script running | < 80 MB | Raycast's Swift shell is ~40 MB. Asyar and Raycast v2 sit around 400 MB. |
| Cold start → hotkey works | < 400 ms | Asyar measured itself at 585 ms and Raycast at 2129 ms. |
| Idle CPU after the panel has been hidden for 60 s | < 0.2%, and no display-link / 60 Hz timer | A GPU toolkit that spins while hidden fails this even if frames are cheap. |
| Notarized DMG, no extensions | < 30 MB | Asyar 68 MB, Raycast 223 MB, same Asyar table. |

If a spike misses the idle-RSS or idle-CPU number, the stack is wrong. Change the stack before writing features.

---

## 1. Prior art

What is loved is a habit plus a way to add one more command without forking the app. What is abandoned is a beautiful window with no plugin story, maintained by one person, usually after a GUI-framework rewrite. Speed is necessary and, by itself, not why anyone stays.

### The commercial bar

**Spotlight, macOS 26 and 27.** Tahoe's Spotlight grew actions via App Intents, in-app actions, and a clipboard history ([secondary], <https://appleinsider.com/inside/macos-tahoe>, page current as of 2026-11-21). A September 2026 write-up of macOS 27 "Golden Gate" says the Spotlight field and the Siri field are the same surface, labeled "Search or Ask", with category chips for Apps and the rest ([secondary], <https://geeksmodo.com/spotlight-shortcuts-mac/>). Treat the 27 details as a secondary report, not an Apple note. The strategic fact does not depend on the exact label: Apple now ships clipboard history and a prompt box on the same shortcut. A new launcher that is "Spotlight with a calculator" is late. The gaps Apple still leaves, and that Alfred and Raycast still charge for in practice, are: a workflow you can version in git, snippets with placeholders, a window switcher with frecency you control, local-only behavior with no account, and commands that run a binary you wrote this afternoon.

**Raycast.** v1 was Swift and AppKit, with Node for extensions. v2 (deep dive 2026-05-14) is four layers: a Swift/AppKit host on macOS and a C#/WPF host on Windows, a React UI in the system web view (`WKWebView` / WebView2), a long-lived Node process, and a Rust core for the file index and shared data ([secondary], same deep dive). They say they ruled out a fully native UI early, and they publish the cost. With the main window hidden: WebContent ~120–200 MB, Node ~150–200 MB, Swift shell ~40 MB, WebKit GPU ~18 MB, networking ~12 MB. An empty WebView is already ~50 MB and a bare Node process ~12 MB. v1 sat around 200–300 MB in normal use; v2 around 350–450 MB. By 2.6 (2026-10-05 changelog) they had cut host memory about in half and extension startup memory about 30% (<https://www.raycast.com/changelog/windows>). The Rust indexer exists because they left Spotlight to be cross-platform, not because Spotlight is a bad index on a Mac.

Raycast is loved for the store and the extension API (`List`, `Grid`, `Detail`, `Form`, `ActionPanel`), which runs React on Node and serializes a view tree. The 2023 API post is still the right description of that bet: <https://www.raycast.com/blog/how-raycast-api-extensions-work>. It is not loved because the window paints in 20 ms. People tolerate hundreds of megabytes for the store.

**Alfred.** Still the native Cocoa reference. Secondary 2026 pieces still talk about Alfred 5 and its workflow model (script filters, hotkeys, snippets behind the Powerpack). I did not open alfredapp.com today, so I am not quoting a version number or an RAM figure for it. The durable lesson is the extension shape: a subprocess speaks JSON or XML, the app draws the rows, and power users will write a 30-line script when they will not write a React component. That shape is fifteen years old and still the right v1.

### Open source that is actually alive

**Vicinae** is the open-source project that won the "Raycast, but open" niche. C++ and Qt, GPL-3.0, about 10.6k stars. v0.20.0 (2026-02-26) rewrote the UI from QWidgets to Qt Quick for GPU rendering and scrolling (<https://newreleases.io/project/github/vicinaehq/vicinae/release/v0.20.0>). v0.30.0 (2026-11-07) added optional AI chat and speech-to-text; v0.30.1 shipped around 2026-11-15 ([searched], <https://newreleases.io/project/github/vicinaehq/vicinae/release/v0.30.0>, <https://openmacapps.com/apps/vicinae/>). Linux is the primary target, with Wayland and X11; macOS is a notarized Apple Silicon beta since 0.23, requiring macOS 14.4+. The process model is a server plus a Qt client. Extensions are TypeScript and React, executed by Node workers, producing a serialized UI tree that the C++ core draws. There is no browser in the extension path (<https://docs.vicinae.com/extensions/introduction>). The API tracks Raycast's on purpose. Clipboard storage has been described as SQLCipher-backed ([secondary], a 2026-10-25 architecture overview). This is a full-time-shaped codebase. Do not compete with it on extension compatibility.

**Asyar** is the existence proof for "Rust + Tauri can feel faster than Raycast," and the existence proof that the memory does not follow. Tauri v2, Svelte 5, GPL-3.0 on the repo (the README also says "GPL/MIT" in a comparison cell; trust the repo license until the `LICENSE` file is read). About 1k stars, 2,579 commits on `main`, actively developed, covered by MakeUseOf on 2026-11-20 (<https://www.makeuseof.com/asyar-open-source-raycast-alternative-launcher/>, <https://github.com/Xoshbin/asyar>). Extensions run in iframes with a small SDK and a store of its own. It does not claim to run Raycast extensions unmodified; it imports Raycast settings. Its published bench (reproduce with `benchmarks/bench.sh`):

| | Asyar 0.1.1-49 | Raycast 2.6.2.0 |
| --- | --- | --- |
| Hotkey → visible, median of 15 | 13.9 ms | 22.2 ms |
| p95 / p99 | 17.3 / 20.5 ms | 32.0 / 32.5 ms |
| Cold start → usable | 585 ms | 2129 ms |
| Idle RSS, all processes | 419 MB | 452 MB |
| Deep-idle RSS | 664 MB | 680 MB |
| On-disk size | 68 MB | 223 MB |

Same machine: M4 Max, 36 GB, macOS 27.0.1, 2026-10-04. Hotkey latency is a solved problem for a resident Tauri app. Idle memory is not. Asyar also documents the Wayland hole: a background process cannot grab a global hotkey, so the compositor has to bind a key to `asyar-summon`. That is another reason "cross-platform from day one" is not free.

**Floe** (`thaw-app/Floe`, not the unrelated menu-bar app `wudb/Floe`) is a macOS launcher under AGPL-3.0. The repo is an Xcode project, Swift sources, a `Vendor/FloeRust` directory, and a `runtime/` directory. The README says it runs Raycast extensions unmodified, plus snippets, emoji, files, a calculator, and clipboard history (<https://github.com/thaw-app/Floe>). About 211 stars and 244 commits on `development` when opened today. I did not find a release version worth citing. It matters because the people pursuing Raycast compatibility on the Mac, in the open, put the shell in Swift/AppKit and the Rust in a library. The RustCast maintainer's README now points contributors at Floe.

**Albert** is the long-lived counterexample to "you must rewrite the UI." C++/Qt6, plugin repo activity through 2026-11-03, latest release I saw listed as v34.0.10 on 2026-02-13, about 8k stars (<https://github.com/albertlauncher/albert>). Extensions are native C++ plugins or embedded Python; the query API is `QueryHandler` / `GlobalQueryHandler` / `FallbackHandler` returning `Item`s (<https://albertlauncher.github.io/extension/>, docs touched 2026-10-26). Linux-first. The lesson is the API, which is boring and stable, not the toolkit.

**PowerToys Command Palette** is Windows, WinUI, inside PowerToys, and still getting crash fixes on 2026-11-20 (<https://github.com/microsoft/PowerToys>). It replaced PowerToys Run as the launcher people mean. A 2026-05-01 XDA piece notes that serious file search is an Everything extension, because the built-in search feels slow (<https://www.xda-developers.com/powertoys-finally-has-launcher-thats-as-good-as-wox-or-flow/>). On Windows the winning move is "plug in a real index." On macOS that index is already Spotlight.

**Flow Launcher** remains the Windows plugin launcher (apps, files, web, community plugins). **Wox** is described in a 2026-11-20 roundup as a GPL cross-platform launcher (<https://www.hechangquan.com/best-windows-launchers-powertoys-flow-listary-wox/>). I did not open the Wox repo today, so I am not asserting its 2026 language. **Ulauncher** is still the Python/GTK name in Linux roundups. None of these are a template for a Mac panel. Flow's "plugin is a process that returns results" is the same idea as Alfred's script filter, and it is the idea to copy.

### Rust launchers, and why they stalled

| Project | Stack, as verified | State on 2026-11-21 | Lesson |
| --- | --- | --- | --- |
| Loungy | GPUI. Started on Tauri, left because "moving info from rust to the webview" was miserable. [searched] <https://github.com/MatthiasGrandl/loungy> | Archived by the owner on 2026-08-25. README points at Gauntlet and Vicinae for plugins. ~1.7k stars. | GPUI can paint a launcher. It cannot grow a plugin system by itself, and a hobby dies when life happens. The author did not blame GPUI. |
| Gauntlet | Rust, iced for the shell, Deno for plugins, a custom React reconciler, no Electron. Plugins are git URLs with explicit Deno permissions. [searched] <https://github.com/project-gauntlet/gauntlet> | README: "The project is no longer being developed." Last release page I opened was v21, dated 16 Aug, year not printed on that listing. ~822 stars. | The most ambitious Rust plugin design in this space. Died with the author. A React reconciler plus a JS runtime is a second product. |
| Flare | Rust plus a Node/pnpm frontend, Raycast API compatibility, Linux. [searched] <https://github.com/ByteAtATime/flare> | Archived 2026-09-30. Author: high-school proof of concept, then burnout. v0.1.0 was the latest release listed. | "Raycast-compatible" is a treadmill. Compatibility without a maintainer is a demo. |
| RustCast (MystikoLab) | iced 0.14, `objc2` 0.6, `nucleo-matcher` 0.3.1, `global-hotkey` 0.7, `rusqlite`, TOML config. [searched] Cargo.toml and README via GitHub | README: no longer maintained. Maintainer points at Floe, and at closed-source Tuna. Latest release I saw: v0.8.5 on 31 Jul (year not on the listing). | The closest ancestor of this project. iced plus a bit of AppKit was enough to ship apps, clipboard, emoji, and scripts, and not enough to keep the author once Raycast-compatible work existed elsewhere. |
| Asyar | Tauri v2 + Svelte 5 | Alive, see above | Memory is the tax. |
| Kunkun | Tauri, Svelte, TypeScript extensions, own store. [searched] <https://github.com/kunkunsh/kunkun> | README still says early WIP. ~1.3k stars. I did not confirm a 2026 release tag. | Another Tauri launcher that bought UI speed with a web stack and then faced the store problem. |
| Sherlock | Rust, GTK4, GPL-3.0. Wayland, Hyprland-oriented. [secondary] <https://www.linuxlinks.com/sherlock-lightweight-highly-configurable-application-launcher/> (2026-08-10) | Alive as a Linux niche app. | GTK4 is a fine Linux shell and a bad Mac shell. |
| Walker | Rust, GTK4, plus a background service called Elephant that feeds providers (apps, files, clipboard, calc, emoji). [secondary] <https://linuxstans.com/best-linux-app-launchers/> (2026-10-15) | Alive on Wayland. | The daemon split pays off when providers are slow and the UI toolkit is separate. Copy the trait boundary, not the process split, until something crashes the panel. |
| Anyrun | Rust, GTK4 layer-shell, plugins as `cdylib` behind `abi_stable` 0.11. Repo updated 2026-11-11. GPL-3.0. ~1.3k stars. [searched] <https://github.com/anyrun-org/anyrun> | Alive, small, Wayland-native. | Dynamic libraries work for Arch users who rebuild. On a notarized Mac they fight the hardened runtime. See section 5. |
| nex-launch | Rust, Windows, crates.io. Listings showed 6.4.0 (2026-05-17) and 2.1.0 (2026-06-20); the versioning on the page is messy. [searched] <https://lib.rs/crates/nex-launch> | A Windows launcher, not a Mac reference. | Ignore for this project. |

**Sol.** Nothing current turned up under that name in today's searches. I am not going to invent one.

The pattern is blunt. Rust launchers die in three ways: the maintainer's life changes (Loungy, Flare, RustCast, Gauntlet), the project spends its life on a JS/React compatibility layer and never becomes a daily driver (Flare, Gauntlet), or it picks a cross-platform web view and then is merely a smaller Raycast (Asyar is the one that survived this, and its RAM chart shows the ceiling). Vicinae survived by being C++, GPL, and obsessed with the extension API, with Linux as the home platform. That path is taken.

---

## 2. UI stack

**Use AppKit through `objc2` 0.6.5 (2026-10-05) and `objc2-app-kit` 0.3.2.** The app-kit crate's 0.3.2 release is dated 2025-10-04 on lib.rs, and docs.rs was still building that version on 2026-10-10 ([searched], <https://lib.rs/crates/objc2>, <https://docs.rs/crate/objc2-app-kit/latest/builds>). Pin those, and re-check `objc2-app-kit` on the day the workspace is created in case 0.4 has appeared.

The search field is `NSTextField` or `NSSearchField`. The results are an `NSTableView` with a fixed row height, or a layer-backed `NSView` only if the table cannot hit the frame budget. The background is `NSVisualEffectView`. The window is an `NSPanel`. Icons are `NSImage`, already in the system cache.

This is the stack because the product is a windowing behavior, not a widget demo. The behaviors in section 3 (non-activating, all Spaces, over fullscreen, focus restored, IME, VoiceOver, vibrancy) are properties of AppKit. Every Rust GUI toolkit re-implements a subset of them on top of `winit` and then files bugs for the rest. Loungy, Gauntlet, and RustCast are the field evidence.

### Why the others lose

**Tauri 2.12.7 (2026-11-18)** is a good application framework ([searched], <https://v2.tauri.app/release/tauri/v2.12.7>). It is the wrong one here. Asyar already ran the experiment on macOS 27 and landed at ~419 MB idle. Raycast's own engineers attribute ~120–200 MB to a hidden `WKWebView` before you have drawn a list. You also inherit WebKit's first-frame flash, emoji-font warmup, and the need for a JS frontend to talk to Rust for every keystroke, which is the pain that pushed Loungy off Tauri. Tauri 3 is alpha (3.0.0-alpha.6 on 2026-11-06). Do not start on an alpha. Rich extension UI is the one argument in Tauri's favor, and section 5 rejects that argument for v1.

**GPUI** is the only Rust toolkit I would accept as a fallback, and only after the AppKit spike fails a latency budget for a reason that is AppKit's fault. Text layout and frame time are proven inside Zed. As of today the crates you can actually depend on are unofficial: `gpui-unofficial` 1.29.3 (2026-11-20), tracking Zed tags, and Longbridge's `gpui-component` 0.10.0 (2026-11-16) with a large component set and an optional JS extension runtime ([searched], <https://lib.rs/crates/gpui-unofficial>, <https://lib.rs/crates/gpui-component>). Upstream GPUI is still described as pre-1.0 with breaking changes. There is also `gpui-x` 0.2.6 and GPUI-CE 0.3.0. Three republish lines is the churn story. Loungy shipped a launcher on GPUI and still died without plugins. GPUI will not hand you `NSWindowStyleMaskNonactivatingPanel`. A hidden GPUI window that keeps a wgpu surface awake will fail the idle-CPU budget. I did not find a 2026 report of GPUI driving `NSVisualEffectView`; assume it cannot, and that a GPUI panel looks like Zed, not like Spotlight.

**iced 0.14.0 (2025-12-07)** added IME, headless tests, and hot reload ([searched], <https://www.phoronix.com/news/Iced-0.14-Rust-GUI-LIbrary>, <https://github.com/iced-rs/iced/releases/latest>). The repo moved on 2026-11-19; the latest release is still 0.14, with a handful of commits on that branch. RustCast shipped on 0.14 and the maintainer left. COSMIC is the real user, and COSMIC is a desktop shell, not a 16 ms panel. I did not see AccessKit in the 0.14 release notes. Vibrancy is not a feature. Gauntlet's death is not iced's fault, but iced did not save that project from the plugin swamp, and it will not get you Spotlight's window. Lose.

**egui 0.35** ("25 Jun" on the release page, year not printed; 286 commits on main since) improved IME and grew an inspection protocol ([searched], <https://github.com/emilk/egui/releases/tag/0.35.0>). Immediate mode is the wrong model for a virtualized, accessible list with a stable text caret. AccessKit exists around egui and is still a second tree you maintain. No vibrancy. Fine for a debug HUD, not for the product window.

**Slint 1.19.0 (2026-11-16)** is the most polished declarative option: editions, backdrop blur, animated images, a real release train ([searched], <https://slint.dev/blog/slint-1.19-released>, <https://crates.io/crates/slint>). The license is `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`. A permissive launcher cannot take a GPL UI toolkit, and a royalty license in the core is a permanent tax. Blur in Slint is also not AppKit vibrancy; it is a renderer effect inside Slint's window. Lose on license and on window semantics.

**Dioxus native / Blitz, Makepad, Xilem, Freya.** I am not staking the panel on any of them. A 2026 ecosystem note (secondary, a research markdown that itself cites Cargo.toml lines) describes Dioxus 0.7 / Blitz rendering HTML with Stylo and Vello, Makepad 2.0 on a `dev` branch with weak AccessKit, Xilem still the Linebender research line, Freya small. I did not open each repo's release page today. They lose for the same structural reason even if the version numbers have moved: none of them is an `NSPanel`, and all of them are a second research project. Revisit Blitz only if, in a later year, you decide extension UI is HTML and you refuse a WebView. Not now.

### Judged against the actual requirements

- **Cold start and hotkey latency.** A resident AppKit process that has already built its panel and called `orderOut` should show it in one frame. No toolkit helps if you create the window on the hotkey. Create it at launch, keep it hidden.
- **Per-keystroke frame time.** An `NSTableView` reload of 20 visible rows is well inside 8 ms. If it is not, you are doing something else on the main thread (decoding icons, running the matcher synchronously on a huge corpus, waiting on a script). Fix that. Do not change toolkits.
- **Idle memory.** AppKit plus a SQLite cache should land near Raycast's ~40 MB shell, with headroom up to the 80 MB budget for the image cache. wgpu, WebKit, and Node blow the budget by construction.
- **Text, IME, emoji, RTL.** `NSTextField` gets these from the system. This is the requirement that disqualifies a custom text editor in a GPU toolkit for v1. Zed's GPUI text is excellent and is still a year of bugs if you own it yourself.
- **Accessibility.** An AppKit text field and table are VoiceOver elements. AccessKit (`accesskit` 0.25.2, 2026-10-25) is what you adopt when you leave AppKit. Staying in AppKit is the accessibility strategy.
- **List virtualization.** `NSTableView` is the virtualized list.
- **Icons.** `NSWorkspace` icon for a path, held in an `NSCache` keyed by bundle URL plus a generation. Do not decode PNG icons on the keystroke.
- **Vibrancy.** `NSVisualEffectView` with a sidebar or popover material. Custom views that draw their own pixels must override `allowsVibrancy` or they look flat on top of the material ([platform], and <https://developer.apple.com/documentation/appkit/nsvisualeffectview>).
- **Theming.** Material (light/dark/system) plus a few fonts and an accent. Not a theme engine. Not CSS.
- **API churn.** AppKit's panel API is older than most of the Rust GUI ecosystem and will outlive it. `objc2` churns, but the calls you need (`styleMask`, `collectionBehavior`, `level`, `orderFrontRegardless`) are stable wrappers.
- **Rich extension UI.** AppKit can draw `List`, `Detail`, `Form`, and `Grid` as native views fed by a JSON view model. It cannot host an arbitrary React tree. That limitation is the point. See section 5.

Escape hatch, if the week-1 panel spike cannot show over fullscreen Spaces without stealing focus: a thin Swift shell (this is Floe's shape) that links a Rust static library for match and providers. Still AppKit. Not GPUI, not Tauri. Decide that at the end of week 1, in writing, and do not revisit for a year.

---

## 3. macOS integration

The shell is one process, `LSUIElement` = true, activation policy `Accessory`, so there is no Dock icon. A status item exists so the user can quit and open settings; hiding it is a config flag, default visible. [platform]

### The panel

Spike this before any feature. The combination that Spotlight-class tools converge on:

- `NSPanel` with `NSWindowStyleMaskNonactivatingPanel`, borderless, full-size content view.
- `collectionBehavior`: `CanJoinAllSpaces | FullScreenAuxiliary | Stationary | IgnoresCycle`.
- `level` at `NSFloatingWindowLevel`, raised toward `NSStatusWindowLevel` only if a fullscreen Space covers it. Higher levels fight menu bar and screenshot UI, so start low and raise once.
- `hidesOnDeactivate = false`.
- Show with `orderFrontRegardless` and make the panel key so the text field accepts keys, without activating the application. `NonactivatingPanel` is what makes "key panel, app not active" possible. [platform]
- On dismiss (`Esc`, focus loss, successful launch): `orderOut`. Do not activate the previous app; it never deactivated. That is the whole trick.
- No show animation. Asyar's review credits "no open animation" for beating Spotlight's feel. An 80 ms fade is a setting, default off.

Clicks in the panel must not bring a main window forward, because there is no main window. `tauri-nspanel` discussion threads are full of people discovering this late (<https://github.com/orgs/tauri-apps/discussions/15251>). You will not be in Tauri, so you set the style mask yourself.

### Global hotkey

Use Carbon `RegisterEventHotKey`, via `global-hotkey` 0.10.0 (2026-11-09, <https://lib.rs/crates/global-hotkey>). The crate's macOS requirement is an event loop on the main thread, which an AppKit app already has. The 0.10.0 changelog commit is "fix(macos): register hotkeys exclusively." I tried to open `src/platform_impl/macos.rs` at tag v0.10.0 and got a 404, so I am not quoting its Carbon call line by line. The explicit bindings live in `carbonhotkey` 0.5.2 (2026-06-06), whose README states the important constraint: `RegisterEventHotKey` does not need Accessibility permission, and it only does modifier-plus-one-key (<https://lib.rs/crates/carbonhotkey>). Prefer `global-hotkey` because it is the maintained cross-platform crate (about 1.3M downloads a month) and it already sits on `objc2`. Drop down to a direct Carbon call only if exclusive registration or the Spotlight key combination misbehaves.

Do not use a `CGEventTap` for the summon hotkey. A tap sees every key, requires Accessibility, and is the API that looks like a keylogger. Reserve a tap for a future text-expansion mode, behind a separate, honestly worded permission, default off.

Sequoia intentionally rejected hotkeys whose only modifiers were Shift and Option (`eventInternalErr` / -9868), then 15.2 restored Option and Option-Shift. Command or Control was the safe subset in between. Apple's note is on the developer forums: <https://developer.apple.com/forums/thread/763878>. Default binding should include Command. On macOS 27, verify Option-Space in the spike rather than assuming 15.2's reversal still holds.

You cannot programmatically unbind Spotlight's Command-Space. [platform] Onboarding offers a button that opens the Keyboard Shortcuts pane, and tells the user to clear Spotlight's shortcut if they want yours to be Command-Space. Do not steal it silently. Register your default as Command-Space only after they confirm, and fall back to Option-Space.

`global-hotkey` on Linux is X11 only. That is acceptable because the Linux shell is out of scope. It is also a warning against pretending this crate is the cross-platform story.

### Launch at login

`SMAppService.mainApp` (macOS 13+) is the supported API: <https://developer.apple.com/documentation/servicemanagement/smappservice>. It shows the system prompt and a Login Items entry, and it leaves with the app. Call it through `objc2`. There is a 2025 crate, `smappservice-rs`, from a Tauri app's write-up (<https://www.gethopp.app/blog/rust-app-start-on-login>); I did not re-check that crate's 2026 status, so do not depend on it. Tauri's autostart plugin is irrelevant if you are not in Tauri.

### Apps

Scan with Launch Services (`NSWorkspace` / `LSCopyApplicationURLsForBundleIdentifier` and the application URL enumerator), not by walking `/Applications` only. Users have apps in `~/Applications`, on other volumes, and inside wrappers. [platform] Watch with a coarse refresh: re-enumerate on `NSWorkspace.didLaunchApplicationNotification` is the wrong signal; observe directory changes on the application directories plus a refresh when the panel opens if the last scan is older than a few minutes. A full rescan of a few hundred bundles is cheap next to fuzzy match.

Icons: `NSWorkspace.icon(forFile:)`, cache the `NSImage` and do not convert to PNG on the query path. RustCast's dependency list includes `icns` and `image`; that is the slow path. Use it only if you ever draw with a toolkit that cannot take an `NSImage`.

### Files

Query the Spotlight index. Do not build one.

`NSMetadataQuery` gives live, scoped, cancellable results and belongs in-process. `mdfind` is the same index from a subprocess and is the right prototype on day one because it is a shell command you can time. Move to `NSMetadataQuery` when you need cancellation and incremental batches without parsing a child process. [platform]

Raycast's Rust indexer is a cross-platform decision, documented as leaving a Spotlight dependency (deep dive, secondary summaries). You are macOS-only. `mds` is already running, already respects TCC, and already has the user's file corpus. An FSEvents plus Tantivy index duplicates that, asks for Full Disk Access to be useful, and is several months of work before it is as good as `mdfind` on day one. Tantivy 0.27.0 (2026-10-17, <https://crates.io/crates/tantivy>) is the tool you want later for clipboard and snippet search if SQLite FTS5 falls over. It is not the file index.

Scope v1 file search to names, in the user's home directory, kind filters optional (`kind:pdf`). Content search stays Spotlight's job, reachable by a fallback action "Search in Spotlight" that passes the query through. That one action prevents a year of indexer work.

### Permissions, in the order you should request them

Request nothing at first launch except the hotkey instructions.

| Permission | When | Why |
| --- | --- | --- |
| None | App launch, calculator, emoji, snippets invoked from the panel, quicklinks, script commands the user installed | The daily loop. |
| None for recording | Clipboard history via `NSPasteboard` `changeCount` polling | Polling the general pasteboard does not need Accessibility. [platform] |
| Accessibility | Paste-back into the previous app (synthesized Command-V is unreliable in some apps without it), window list and raise via the AX API, menu-bar search of the front app | One prompt, one settings page, features degrade with a clear empty state if denied. |
| Automation (AppleEvents) | Only if a script or a built-in drives a specific app by AppleScript | Per-target prompts. Avoid in core. |
| Screen Recording | Only if you insist on window thumbnails or on `CGWindowList` names | Since Catalina, `CGWindowListCopyWindowInfo` returns windows but withholds `kCGWindowName` without the permission ([secondary], <https://figment.so/blog/mac-screen-recording-xcuitest>). A July 2026 developer-forum post claims macOS 27 populates `kCGWindowOwnerName` even without the permission, and Apple replied that inferring permission from which keys are present is undocumented (<https://developer.apple.com/forums/thread/839069>). Do not build a feature on that. Prefer AX for "switch to window," which needs Accessibility, not Screen Recording. |
| Full Disk Access | Never in v1 | Needed to crawl protected directories yourself. `mdfind` returns what Spotlight can already show the user. Skip the crawl and you skip the prompt. |
| Input Monitoring | Never, unless you later build system-wide text expansion | Different trust level from Accessibility. Do not bundle them. |

Password managers mark copies with pasteboard types `org.nspasteboard.ConcealedType` and `org.nspasteboard.AutoGeneratedType`. [memory, the nspasteboard.org convention; confirm the constants in the clipboard spike.] Drop those entries. Vicinae's GNOME companion grew the same idea the hard way (release notes for the extension, 2026-10).

### Window management

AX UI element for the focused app, walk windows, raise by AX. Titles and "frontmost" are Accessibility-gated and break across releases. Ship a switcher (type, return, that window raises), not a tiler. Tiling is a different product (the RustCast author split it into Sxitch). Rectangle, AeroSpace, and yabai exist. Refusing to be a window manager is a feature.

### Signing, notarization, sandbox, updates

Distribute outside the Mac App Store. Developer ID, hardened runtime, `notarytool`, staple the DMG. Hardened runtime is required for notarization. [platform]

The App Sandbox is not feasible for the product you actually want. A sandboxed app cannot freely exec user scripts, observe other processes' windows, or act as a general launcher without a list of temporary exceptions that reviewers reject and that defeat the sandbox. Sparkle 2 can update sandboxed apps (<https://sparkle-project.org/documentation/sandboxing/>, docs touched 2026-11-17), which is interesting and not a reason to sandbox this one. The Sparkle repo itself was active 2026-11-19 (<https://github.com/sparkle-project/Sparkle>).

Do not disable library validation. That entitlement is what unsigned dylib plugins need, and it is a malware signal. It is also why section 5 rejects `abi_stable` plugins on macOS.

Updates: Homebrew cask for the first users. Sparkle in the phase that has a real appcast, driven via `objc2` against `Sparkle.framework`, EdDSA plus code-signature verification as Sparkle 2 already does. Do not write a homegrown updater that copies a bundle over itself.

A week-2 task is to notarize an empty `LSUIElement` panel that registers a hotkey. Notarization surprises are schedule surprises. Find them before there is a UI to blame.

---

## 4. Search and ranking

### Matcher

**Default matcher: `nucleo-matcher` 0.3.1.** It was published 2024-02-20 and has not had a release since. That is a virtue. The authors call the core finished, Helix ships it, and docs.rs was still building it in October 2026 ([searched], <https://crates.io/crates/nucleo-matcher/0.3.1>). License MPL-2.0, which is file-level copyleft: acceptable if you do not modify the crate, annoying for some downstreams. It uses fzf's scoring, including word-boundary and camelCase bonuses, so a separate acronym pass is unnecessary for Latin text. On the benchmark they publish, nucleo is often about six times faster than skim on low-selectivity queries. For a launcher corpus this is already sub-millisecond.

**Typo pass: `neo_frizbee` 0.13.3 (2026-09-21), only as a second pass.** The crate claims, with typos disabled, about 4× nucleo and about 20× on unicode, using SIMD Smith-Waterman (<https://crates.io/crates/neo_frizbee/0.13.3>). An earlier `frizbee` 0.9.0 (2026-04-04) claimed ~1.8×. I did not re-run those benches today. The algorithm note that matters: nucleo and fzf prefilter to "all needle characters present, in order," which rejects typos; frizbee can allow `max_typos`. Use that only when the strict pass returns fewer than three hits and the query is at least four characters. A typo-tolerant primary ranker makes `calander` work and makes `sl` match a depressing amount of the filesystem. Quality first, SIMD second.

`fuzzy-matcher` / skim lose on the numbers nucleo already published. `sublime_fuzzy` is a smaller algorithm with a smaller community; I did not open its crate page today and I would not bet the ranker on it. Verify `neo_frizbee`'s license in the spike before linking it. If the license is wrong, skip typos rather than adopt a different primary matcher.

Run the matcher off the main thread on a snapshot of the corpus (an `Arc` swap, not a lock held across scoring). Publish the top 50 plus match ranges back to the table. Highlight with the ranges nucleo returns. Do not re-filter in the view.

### Frecency

SQLite row per result id: `uses`, `last_used_ms`. At query time:

```
age_days = (now - last_used_ms) / 86_400_000
frecency = ln(1 + uses) * exp(-age_days / 14)
final = match_score * (1 + frecency)
```

Exact title match pins to the top. A user-set alias is an exact match on the alias. The half-life of 14 days is a constant in the TOML file, not a research project. This is enough to make "the app I launch daily" beat "the app whose name is a better fuzzy match." Store it in the same SQLite file as everything else. Record a use on launch, not on highlight.

### Non-Latin and layouts

v1: Unicode casefold, strip Latin diacritics, and let nucleo match codepoints. That covers German, French, and the emoji shortcodes you index yourself (the `emojis` crate approach RustCast used).

Pinyin and other transliterations are an extra column on the item (`name_latin`) filled at index time, not a different matcher. Add the column when a real user needs it. Default assumption: the developer types Latin. Keyboard-layout-independent matching (score the query against both the typed characters and the characters those physical keys produce in QWERTY) is a spike, not a feature, until someone demonstrates a layout where the default feels wrong. It is easy to get half-right and ruin ranking.

### Incremental providers and cancellation

Every keystroke bumps a `u64` generation. Providers capture the generation when they start. The UI drops any batch whose generation is older than current. App and command search is synchronous and finishes inside the 8 ms budget, so it has nothing to stream. File search and scripts stream batches and must check the generation between batches.

`mdfind` in v1: kill the child when the generation changes. `NSMetadataQuery`: stop the query. Scripts: kill the process group. Never queue a provider behind another. Apps paint immediately; files append; a slow script cannot freeze the list.

### When a real index is warranted

| Corpus | Structure | Tool |
| --- | --- | --- |
| Apps, commands, emoji, snippets, a few hundred to ~20k | In-memory `Vec`, rebuilt when the scan changes | nucleo over the `Vec` |
| Frecency, clipboard metadata, config | Relational, small | SQLite via `rusqlite` with bundled SQLite. RustCast used 0.40.1; pin a current 0.40 or 0.41 when you create the workspace. |
| Clipboard body search, once history is thousands of rows | Text | SQLite FTS5 in that same file, from the first clipboard schema, so you do not migrate later. |
| File names and file contents | Already an index | Spotlight |
| A future non-macOS port, or clipboard past a few hundred thousand rows where FTS5's p95 exceeds 30 ms | Your index | Tantivy 0.27 |

Do not introduce an FST. The corpora that fit an FST (a static dictionary of names) already fit in a `Vec`, and an FST makes updates and frecency awkward.

### Persistence

One SQLite file in Application Support. WAL mode. Tables: `apps`, `frecency`, `clipboard`, `clipboard_fts`, `snippets`, `scripts`, `kv`. Config is not in SQLite. Config is TOML, so it diffs and so a user can edit it. The GUI writes TOML; it does not keep a parallel store.

No custom binary format. You will want to open the file with the `sqlite3` CLI when something ranks wrong.

---

## 5. Extensions

This is the decision that kills the project if you get it wrong, and the prior art is consistent about how.

**v1 is scripts. A later version is a JSON view protocol over the same transport. WASM is a library feature for untrusted pure functions, not the UI. Raycast compatibility is out.**

### Compared

| Model | Isolation | Startup | DX for the person who will actually extend this | UI | Distribution | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| Script / Alfred script filter. Executable, JSON on stdout. | A process. User privileges. You document that. | Process spawn, single-digit ms on a warm disk for a small binary, worse for a cold Python. | A file and a shebang. Any language. | Rows of title, subtitle, icon, arg. Actions are "run again with this arg." | A directory of files, git-cloneable. | **Ship this.** |
| JSON-RPC over stdio, long-lived process, declarative views (`List`, `Form`, `Detail`, `Grid`). | One process per open command. Kill on close. | Spawn once per command use, not per keystroke. | A bit more than a script, still no SDK in a particular language. | The views you implemented natively. | Same directory, plus a manifest. | **Second extension milestone, after you feel the lack.** |
| WASM component model, wasmtime 51.0.0 (2026-11-20). WIT. | Strong for pure computation. Capability-based. Wasi is still a moving target for filesystem and clocks; wasmtime had several 2026 CVEs around component-model resource handling (for example CVE-2026-126407). You must track their security releases. | A cached compiled module can start in well under a millisecond after the first hit. The engine is not free in binary size. | `wit-bindgen`, a component toolchain, and a language that emits components. Worse than a shell script for the audience that has Alfred workflows today. | None, unless you invent a UI ABI. The moment the component returns a view tree you have built the JSON protocol anyway, plus a compiler. | A `.wasm` file. Nice. | Later, for calculators and converters you do not trust. Not the v1 story. |
| Extism 1.41.0 (about a month before this writing). | WASM with a simpler host API than raw WIT. | Similar. | Better than raw wasmtime, still a plugin SDK. PDK crate `extism-pdk` 1.4.1 is over a year old, which is a smell. | Bytes in, bytes out. You still draw. | A `.wasm` file. | If you do WASM, prototype Extism and wasmtime for one afternoon and pick wasmtime unless Extism is clearly less code. Do not do that afternoon in month one. |
| Embedded Lua (`mlua-sys` 0.13.0, 2026-10-03, so the mlua line is alive; I did not pin the high-level `mlua` version), Rhai 1.27.0 (2026-10-21), Rune, QuickJS, Boa, Deno, V8. | In-process unless you add a process. A panic or a tight loop in Rhai is your problem; a `while true` in Lua is your problem; Deno is a separate runtime and Gauntlet's actual choice. | Excellent, which is the trap. | Another language your users do not particularly want. Albert already offers Python and the users who wanted it are on Albert. | Whatever you expose. An embedded runtime that can touch the view tree will eventually touch the view tree unsafely. | You become a language vendor. | No. Rhai is the least bad of these and it is still a new language for a user who has bash. |
| Native `cdylib` via `abi_stable`, Anyrun's model. | None. Same process, any memory bug is a remote-code-execution bug with extra steps. | Excellent. | Rust only, and `abi_stable` is famously brittle across compiler versions. | Total. | A `.dylib` you cannot load under the hardened runtime without disabling library validation. | No on macOS. This is not a philosophy point. Notarization makes it the wrong mechanism. |
| Raycast API compatibility. React reconciler in Node, Deno, or Bun. Vicinae's model (Node worker, serialized tree, native draw). Gauntlet's model (Deno, custom reconciler, iced widgets). Flare's model (dead). Floe's model (Swift shell, a runtime directory, AGPL). | Vicinae and Raycast run untrusted JS in a side process, which is the right isolation and a large runtime. Raycast's own Node is 150–200 MB. | Seconds of engineering per API method, forever. The Raycast API post explains that they version the API npm package and refuse to run an extension whose API is newer than the app. You would be chasing that version number. | The best DX in the field, and it belongs to Raycast. | The best, and it belongs to Raycast. | Their store, their terms. | **Do not build this.** |

### Raycast compatibility, said plainly

Vicinae, Floe, and the archived Flare project are already spending the compatibility budget. Vicinae is GPL-3.0. Floe is AGPL-3.0. If this launcher is MIT or Apache-2.0, you do not read their extension runtimes. You may read their docs. A clean-room reimplementation of the Raycast API is legally a gray, expensive project (their API package, their store terms, their trademarks) and technically a treadmill: every Raycast release is a bug against you. Asyar made the smarter commercial move and imported settings rather than extensions, and even that is a one-developer product with rough edges (MakeUseOf, 2026-11-20).

The ecosystem leverage is real. It is also a company. A solo developer plus coding agents who starts there will not have a daily driver, which is how Gauntlet and Flare ended.

### What ships when

- **v1.** Script commands. Manifest is the script's front matter or a sidecar TOML: `title`, `subtitle`, `mode` (`list` or `silent`), `icon`. Protocol is Alfred-adjacent JSON: `{ "items": [ { "title", "subtitle", "arg", "icon" } ] }`. The launcher sends the query on stdin or as an argument (pick one in the spike and freeze it). Silent mode runs and pastes or copies. List mode replaces the results. Reload-on-type is a flag, default off, because a script that runs per keystroke will miss the 8 ms budget and that is fine if it is opt-in.
- **v1, also.** A URL scheme so Shortcuts and other scripts can `show`, `search`, and `run`. RustCast already grew `rustcast://open` and `rustcast://show`. Copy the idea.
- **Next, only after three real scripts feel cramped.** A view protocol: the process emits `list`, `detail`, `form`, or `grid` documents with stable item ids and action names. The shell renders them natively and sends events back (`action`, `form-submit`, `search-changed`) as JSON lines tagged with the generation. This is Vicinae's architecture without React, and Albert's item model with a form. It covers the extensions people actually write: pick from a list, fill two fields, see a detail pane.
- **Later, if you accept third-party code you do not trust.** A wasmtime component with a WIT interface of `filter(query) -> scores` and nothing else. No UI, no filesystem, no network, unless a capability is in the manifest and the user approved it. Track wasmtime security releases; 2026 has already produced component-model CVEs.
- **Explicitly not scheduled.** A store, a React reconciler, Node bundled in the app, Deno, and dylibs.

---

## 6. Feature scope

A person switches the day the launcher is faster than what they have and covers the four things they do without thinking: launch, calculate, paste something they copied, and run one personal command. They do not switch for a store they have not filled.

### MVP, the thing you use for five straight days

1. **App launch** with frecency, aliases, and Return to open. Hide the app you are already in, or show it last. This is the habit.
2. **Calculator**, inline, no mode. `evalexpr` is what RustCast shipped (their Cargo.toml pins 13.1.0). Expressions only. Units: a small table (length, weight, temperature, time), not Numbat. Numbat is what Loungy and Gauntlet embedded, and it is a language. Currency: a daily rate file, last-known if offline, never on the critical path. If the rate fetch is slow, the number does not appear, and the app list still does.
3. **Clipboard history** of text and images, local, concealed types dropped, search via FTS5, paste-back best-effort and a copy-to-clipboard fallback that needs no permission.
4. **Emoji and symbols** by shortcode and tags, copy on Return.
5. **Quicklinks.** `g foo` and a TOML list of `keyword = "https://…{query}"`.
6. **System commands** that are just process launches: sleep, lock, empty trash, toggle dark mode, open a Settings pane. No UI framework required.
7. **Snippets** triggered from the launcher: keyword expands into the query or copies. Placeholders `{cursor}`, `{clipboard}`, `{date}`. This is the Alfred feature people pay for, and it is a table in SQLite.
8. **Script commands**, as specified above, at least `list` and `silent`.
9. **Onboarding** for the hotkey and login item. Nothing else.

### Next, after the five-day test

- File name search through Spotlight, streaming, with a "Search in Spotlight" fallback for content.
- Window switcher via AX, degraded cleanly when Accessibility is denied.
- Front-app menu search, also AX. Vicinae added the macOS version of this in 0.28.0. It is a power feature and a permission feature. Do it once the switcher works.
- Deeplinks and a small CLI (`show`, `hide`, `run`).
- Sparkle.
- Themes: material and font. Stop there.

### Refuse

- **Raycast extension compatibility and a store.** Section 5.
- **AI chat as a product.** macOS 27 is growing a prompt in the same UI as search, if the secondary report holds, and Raycast already spends its Pro tier here. A chat pane is how a launcher becomes a subscription. If you want models, a script that reads stdin and writes stdout already covers "fix grammar" without a provider framework. MCP servers in the core are the same trap: Asyar advertises them, and they are a distribution and permission surface (a server that can exec). Revisit only after the five-day test, and only as a script host, not as a chat UI.
- **System-wide text expansion.** That is an event tap, a keylogger-shaped permission, and a support burden. Snippets inside the launcher are in; expansion in other apps is out.
- **A window manager.** No tiling, no zones, no "move to display" matrix beyond what falls out of the switcher for free. Ship a switcher.
- **Browser tabs and bookmarks.** Requires a browser extension, a native host, and per-browser breakage. A quicklink to a search engine is the v1 answer.
- **Calendar.** Permission and API churn, and it does not help the habit.
- **Your own file content index.**
- **Mac App Store.**
- **Accounts, sync, iCloud.** Floe offers iCloud sync of aliases and snippets. It is a fine later feature and a bad early one. Local SQLite and a TOML file can be symlinked by the user who has more than one Mac.
- **Telemetry.** None. No crash uploader. A local log the user can attach to an issue is the whole story.

---

## 7. Cross-platform

**macOS only, until the Mac build has been a daily driver for someone who is not you.**

The architecture that keeps the door open is a pure `core` crate and a `shell-macos` crate. Providers are traits. The macOS shell is the only type that links `objc2`. A Linux port is a new shell, GTK4 with layer-shell, informed by Sherlock, Walker, and Anyrun, and it will have its own hotkey story because Wayland will not let you register a Carbon-style hotkey (Asyar's README is the write-up of that pain). A Windows port is a third shell, and Command Palette plus Flow already occupy it.

What you must not do "to keep the door open":

- Start on Tauri because it has three backends. You would pay Asyar's 419 MB on day one, on the platform you care about, for a Linux port you have not written.
- Abstract the window behind a trait with `show()`, `hide()`, `set_items()` in week one. You do not yet know which panel behaviors matter. Abstract after the second shell exists, not before the first one is good. A trait that hides `collectionBehavior` will be a lie.
- Use `global-hotkey` as evidence the input layer is portable. On Linux it is X11 only.

The portable core is still worth the boundary now, because it is also the test boundary. Ranking, frecency, script protocol, and TOML parsing should run under `cargo test` on Linux CI without a Mac.

---

## 8. Architecture

### Crate layout

```
crates/core          ranking, frecency, provider trait, script protocol, config
crates/providers     apps, calc, clipboard, emoji, snippets, quicklinks, system, scripts
crates/shell-macos   NSPanel, hotkey, login item, Launch Services, mdfind, AX, pasteboard
crates/cli           talks to the resident process over a unix socket
```

One workspace, one binary. The CLI is a subcommand of the same binary (`argv[1] == "show"` connects to the socket, otherwise you are the resident). No daemon, no helper tool, no XPC service in v1.

`providers` may call small macOS functions behind a trait implemented in `shell-macos` (`fn installed_apps()`, `fn pasteboard_changed()`). The trait's test double returns fixtures. Do not put `objc2` in `core`.

### Process and threads

One process. Vicinae's server/client split is justified by a Qt UI that they want to restart, and by Node workers. You have neither. A crash in the panel is a crash in the product; a second process would crash the same way once the UI owns the state. Scripts are already other processes, and they are the untrusted code.

Main thread: AppKit, the hotkey manager, and applying result batches to the table. A tokio runtime on background threads: file queries, clipboard persistence, script supervision, rate refresh. The runtime does not own the panel.

Do not run the fuzzy matcher on the main thread if the corpus can exceed a few thousand items. At a few hundred apps it does not matter. Structure it as a channel from the start so you do not repaint the architecture when clipboard and files join the corpus.

### IPC

Unix socket in `~/Library/Application Support/<app>/run.sock`, with a peer-credential check so only the same uid can connect. Messages: `Show`, `Hide`, `Run(id)`, `Search(query)`. Deeplink handler (`apple-event` for the URL scheme) turns into the same messages. If the socket is down, the URL scheme launches the app and then shows.

### Crash isolation

Scripts run in their own process group with a timeout (default 5 s for a list reload, configurable). A script that dies returns an error row, not a dead launcher. The core never `dlopen`s third-party code.

Inside the process, the risky parsers are clipboard HTML and file paths. Keep them on the background runtime. A panic hook writes a backtrace to the local log and lets the process die. Launchd-style relaunch is not required in v1; `SMAppService` will bring it back at next login, and the status item's absence is the user's signal.

### Config

TOML at `~/.config/<app>/config.toml`, overriding a defaults file you ship. Watched with `notify` or a coarse mtime check when the panel opens. Keys worth having on day one: hotkey, theme material, max clipboard rows, script directory, search engines, alias map, frecency half-life. Secrets do not go in the TOML. There are no secrets in v1.

### Telemetry

None. No network calls in the core except the optional currency rate fetch, default off until the user adds a currency quicklink or enables the calculator's currency flag. Document the one outbound request next to the flag.

### Testing and benchmarks

- `core`: unit tests for ranking, frecency decay, generation cancellation, script JSON parsing, and the "typo pass only under three hits" rule. Table tests with a frozen corpus of a few hundred names, including aliases, CJK, emoji, and RTL sample strings, so a scoring change shows up as a diff.
- Criterion benches in CI (GitHub `macos-14` or a Linux runner; the core does not care) for: 20k short strings, query length 1, 3, and 8; assert p95 under 2 ms for the matcher alone on the CI runner class you pick. CI runners are slower than an M-series laptop. Gate the absolute number on the runner, and keep a second, tighter number for a local Apple Silicon job.
- A local tool, modeled on Asyar's `benchmarks/bench.sh`, that synthesizes the hotkey, times until the panel window is visible, samples RSS of the process, and samples idle wakeups with `powermetrics` if the user has passwordless sudo. Check this in week 1 and again before every phase exit. Do not claim the CI job measures pixels. GitHub's macOS runners are a bad WindowServer.
- An AppKit UI test is optional and brittle. One XCUITest or one `osascript` that shows, types, and asserts the first row is worth more than a framework. Skip it until the five-day test, then add a single smoke test.

---

## 9. Risks, in the order they will actually hurt

1. **The panel behaves wrong over fullscreen Spaces, or it activates the app and the user's editor loses focus.** This is the product. If you get it wrong, nothing else matters. De-risk: days 1–3, a binary that is only the panel and a text field. Test on the built-in display, an external display, a fullscreen app, and a different Space. Exit: hotkey shows, typing works, Esc returns you to the previous app's caret, and the Dock icon never appears.

2. **The hotkey story is annoying.** Command-Space is owned by Spotlight. Option-Shift combinations have been broken before. `global-hotkey` 0.10 might not register exclusively in the way you need. De-risk: same binary, same three days. Try Command-Space after the user clears Spotlight, Option-Space, and a Command-Shift binding. Confirm no Accessibility prompt appears for the hotkey itself.

3. **Idle RSS or idle CPU misses the budget.** A timer, a wgpu surface, an animated blur, or a WebView snuck in. De-risk: week 1, measure with the Asyar method (all processes, window hidden, 60 s quiet). If RSS is over 80 MB or a display link is firing, stop and find the cause before features.

4. **Clipboard history stores a password, or paste-back is flaky.** De-risk: week 1, a logger that prints pasteboard types only (not values) for copies from a password manager, Notes, and a browser. Confirm concealed types show up. Implement drop-on-type. Paste-back is best-effort; the fallback is "item is now on the clipboard."

5. **You start a second GUI stack on day 4 because AppKit lists feel boring.** Loungy's history is "started in Tauri, moved to GPUI, archived." RustCast's history is "iced, then the maintainer went to a Swift Raycast-compatible app." De-risk: write the stack decision, and the idle-memory number, into the repo README at the end of week 1. Change it only if a budget failed.

6. **Extension scope arrives early** ("let's just embed Deno, Gauntlet proved the API"). De-risk: the script protocol is frozen in week 2 as a one-page spec. New ideas go in a `LATER.md`. Agents working on the repo are pointed at that file.

7. **Notarization and the hardened runtime.** De-risk: week 2, notarize the empty panel. Find the entitlement and the bundle-layout mistakes while the bundle is small.

8. **AX window listing is flaky on macOS 27.** The July 2026 forum thread shows window-list metadata already moving. De-risk: a week-2 spike that lists windows of Finder and one Electron app, raises one, and records which permission prompt appeared. If it is worse than "Accessibility prompt, then it works," the switcher slips a phase. It does not block the MVP.

9. **Ranking feels worse than Spotlight on apps, even though it is faster.** Frecency cold-start is empty, so the first day is strictly fuzzy. De-risk: seed frecency from nothing, but put the frontmost app and a hard-coded list of recent `NSWorkspace` launched apps at the top until `uses` is non-zero. [platform: `NSWorkspace` running applications is available; a true "recent apps" API may need the same scan you already do. Confirm in the spike.]

10. **License contamination.** Reading Vicinae or Floe runtime code while claiming MIT/Apache. De-risk: decide the license in week 1 (default below) and write a one-line note in `CONTRIBUTING`: do not copy from GPL or AGPL launchers. Use their docs and your own spikes.

11. **Spotlight queries are slower or less relevant than you remember, and someone proposes an indexer.** De-risk: in the file-search phase, record p95 of `mdfind` on that machine for 1, 3, and 8 character queries. Only open an indexer discussion if p95 is over ~150 ms or the results are unusably wrong. "Raycast uses Tantivy" is not evidence. Raycast left macOS.

---

## 10. Roadmap

One developer, with coding agents. Dates are effort, not calendar religion. Each phase ends when the exit criteria are true, not when the week number arrives.

### Phase 0 — Spikes (week 1 and the start of week 2)

Panel, hotkey, app enumeration, nucleo over that list, Return launches, Esc dismisses, RSS and hotkey latency measured.

Exit: the budgets for hotkey p95, idle RSS, and idle CPU hold on the developer's machine. Focus is restored. Decision note in the README: AppKit stays, or the Swift-shell escape hatch is taken. License is chosen. Script-protocol sketch is one page, not implemented yet.

### Phase 1 — Daily driver (about four weeks)

Frecency, aliases, calculator, emoji, quicklinks, system commands, snippets, clipboard history, TOML, login item, status item, signed and notarized DMG, Homebrew cask instructions.

Exit: the developer uses it as their launcher for five consecutive days. A written list of the Raycast or Alfred commands they still alt-tab away for. That list is the backlog, not a brainstorm.

### Phase 2 — The list from the five-day test, plus the unblocking platform features (about four weeks)

Likely contents, unless the list says otherwise: script commands, Spotlight file names, deeplinks and CLI, window switcher if Accessibility behaved in the spike.

Exit: at least three personal scripts replace real commands the developer used before. File search p95 is recorded. A second person can install the DMG and open an app.

### Phase 3 — Someone else keeps it (about four to six weeks)

Sparkle, menu search if the switcher is solid, the JSON view protocol only if a real script was cramped, onboarding polish, one smoke test.

Exit: one person who is not the developer keeps it installed for a week and can name a reason. Idle budgets still hold with clipboard history populated and three scripts installed.

### Phase 4 — Only what phase 3's user asked for

If they asked for Raycast extensions, the answer is still no; offer the view protocol and a script. If they asked for Linux, start a shell only after `core` has been clean for a month. If they asked for AI, it is a script with a local model, not a pane in the results list.

Stop conditions, which override the schedule: idle RSS over 80 MB, a second UI toolkit "just for settings," a Node or Deno dependency, a dylib plugin, a FSEvents indexer, an account system. Any of those means the project has become Vicinae or Asyar, and those projects already exist.

---

## Open questions

Defaults are what I will assume if nobody answers. They are choices, not hedges.

1. **License.** Default: MIT OR Apache-2.0. If you already know you want to lift code from Vicinae, the default is wrong and the project should be GPL-3.0 on purpose. I am assuming you do not.
2. **Does Command-Space have to work on day one?** Default: offered in onboarding, not taken silently. Option-Space works without the user changing System Settings.
3. **Languages you type every day.** Default: English plus emoji shortcodes. No pinyin column until you say otherwise.
4. **Do you want AI in v1?** Default: no. No API keys, no Ollama, no MCP. A script can shell out later.
5. **Telemetry and crash reports.** Default: none, ever, unless you reopen the question.
6. **Minimum macOS.** Default: 14 Sonoma, so `SMAppService` is old news and you match Vicinae's floor. You develop on 27, and you smoke-test the panel on 26 and 27 because fullscreen and window-list behavior moved.
7. **Distribution.** Default: Developer ID, notarized DMG, Homebrew cask. Not the Mac App Store.
8. **Product name.** Default: do not ship as "rusty-spotlight." Use a working name in the bundle id (`dev.<you>.launcher` is fine) and pick a real one before the first DMG a second person installs.
9. **Which Raycast or Alfred commands do you actually run in a week?** Default assumption: app launch, clipboard, snippets, a window switch, and a handful of scripts. Not calendar, not Jira, not an AI chat. If that assumption is wrong, phase 1 changes and I would want to know before week 3.
10. **Menu bar icon.** Default: visible, with a hide flag.
11. **Swift in the shell.** Default: no Swift. Rust and `objc2` only, with the week-1 escape hatch of a thin Swift shell if the panel spike fails. Not a SwiftUI rewrite.
12. **Window management.** Default: switcher only, and only in phase 2.
13. **File search.** Default: Spotlight names in phase 2, content search delegated back to Spotlight.
14. **Sync.** Default: none. TOML and SQLite on disk.
15. **Currency and network.** Default: calculator has no network until a flag is on.

---

## What I would argue if the other two members disagree

- If they recommend Tauri: Asyar already built it, measured it on macOS 27 against Raycast 2.6, and the idle cost is the same class as the thing you are trying to replace. Hotkey milliseconds are not the scarce resource. Resident memory and Spotlight-grade window behavior are.
- If they recommend GPUI: it is the right toolkit for an editor. Loungy is the evidence it can draw a launcher and the evidence that a launcher on GPUI still needed a plugin system and a maintainer. Unofficial crates.io republishes (1.29.3 vs 0.10.0 vs 0.2.6) are a bad foundation for a one-person app whose windowing requirements are AppKit requirements.
- If they recommend Raycast API compatibility: Vicinae (GPL, C++, Qt, Node, 10k stars, shipping 0.30 this month) and Floe (AGPL, Swift shell) are ahead by a year or more. Flare and Gauntlet are the corpses of that strategy inside the Rust community. A script filter is how Alfred stayed relevant without a store.
- If they recommend a Rust file indexer: that is Raycast's cross-platform tax. You are not cross-platform yet. `mdfind` is the indexer.
- If they recommend WASM or Deno first: wasmtime 51 is real, and a script still ships in a weekend. Embed a language and you will spend the month on the language.

The interesting risk in my own recommendation is boredom. An `NSTableView` will feel like less of a product than Asyar's screenshots, and an agent session will be tempted to "just add" a web view for settings. The budgets in the verdict are there so that temptation has a number attached.

---

## Source index

Primary or near-primary pages opened or returned in search on 2026-11-21:

- Raycast deep dive (cited via consistent secondary quotations of the 2026-05-14 post): <https://www.raycast.com/blog/a-technical-deep-dive-into-the-new-raycast>
- Raycast memory write-up quoting those figures: <https://www.merlin.computer/blog/raycast-2-memory-usage-mac-2026>
- Raycast Windows changelog (2.6 memory notes): <https://www.raycast.com/changelog/windows>
- Raycast extension API (2023, still the model): <https://www.raycast.com/blog/how-raycast-api-extensions-work>
- Asyar README and bench: <https://github.com/Xoshbin/asyar>
- MakeUseOf on Asyar, 2026-11-20: <https://www.makeuseof.com/asyar-open-source-raycast-alternative-launcher/>
- Vicinae releases and org: <https://github.com/vicinaehq/vicinae>, <https://newreleases.io/project/github/vicinaehq/vicinae/release/v0.30.0>, <https://openmacapps.com/apps/vicinae/>
- Vicinae extension docs: <https://docs.vicinae.com/extensions/introduction>
- Loungy: <https://github.com/MatthiasGrandl/loungy>
- Gauntlet: <https://github.com/project-gauntlet/gauntlet>
- Flare: <https://github.com/ByteAtATime/flare>
- RustCast: <https://rustcast.app/>, <https://github.com/unsecretised/rustcast/blob/master/README.md>, Cargo.toml at `MystikoLab/rustcast`
- Floe: <https://github.com/thaw-app/Floe>
- Albert: <https://github.com/albertlauncher/albert>, <https://albertlauncher.github.io/extension/>
- Anyrun: <https://github.com/anyrun-org/anyrun>
- PowerToys: <https://github.com/microsoft/PowerToys>
- Tauri 2.12.7: <https://v2.tauri.app/release/tauri/v2.12.7>
- Slint 1.19: <https://slint.dev/blog/slint-1.19-released>, <https://crates.io/crates/slint>
- iced 0.14: <https://github.com/iced-rs/iced/releases/latest>
- egui 0.35: <https://github.com/emilk/egui/releases/tag/0.35.0>
- GPUI crates: <https://lib.rs/crates/gpui-unofficial>, <https://lib.rs/crates/gpui-component>
- objc2: <https://lib.rs/crates/objc2>, <https://crates.io/crates/objc2/versions>, <https://docs.rs/crate/objc2-app-kit/latest/builds>
- global-hotkey 0.10.0: <https://lib.rs/crates/global-hotkey>, <https://github.com/tauri-apps/global-hotkey>
- carbonhotkey: <https://lib.rs/crates/carbonhotkey>
- RegisterEventHotKey Sequoia thread: <https://developer.apple.com/forums/thread/763878>
- CGWindowList permission thread (macOS 27): <https://developer.apple.com/forums/thread/839069>
- SMAppService: <https://developer.apple.com/documentation/servicemanagement/smappservice>
- Sparkle: <https://github.com/sparkle-project/Sparkle>, <https://sparkle-project.org/documentation/sandboxing/>
- nucleo-matcher 0.3.1: <https://crates.io/crates/nucleo-matcher/0.3.1>
- neo_frizbee 0.13.3: <https://crates.io/crates/neo_frizbee/0.13.3>
- tantivy 0.27.0: <https://crates.io/crates/tantivy>
- wasmtime 51 (2026-11-20): <https://newreleases.io/project/github/bytecodealliance/wasmtime/release/v51.0.0>
- extism 1.41.0 search listing: <https://crates.io/search?q=extism>
- rhai 1.27.0: <https://crates.io/crates/rhai>
- NSVisualEffectView: <https://developer.apple.com/documentation/appkit/nsvisualeffectview>
- macOS Tahoe Spotlight (secondary): <https://appleinsider.com/inside/macos-tahoe>
- macOS 27 Spotlight/Siri (secondary): <https://geeksmodo.com/spotlight-shortcuts-mac/>

Ignored as contradicted: <https://johal.in/benchmark-raycast-20-vs-alfred-50-launch-speed>.
