# Requirements: a fast Rust launcher for macOS

Status: version 4, 2026-11-21. Scope narrowed by the author; design measured from Spotlight; revised after council round 3, which reviewed this scope, the design and the first code.
Working name: `rusty-spotlight`.
Companion documents: [PLAN.md](PLAN.md), council reports and critiques in [council/](council/) (round 3: [Grok](council/round3-grok.md), [Astra](council/round3-astra.md)).

Only the few numbers in PLAN.md section 0 have been measured, informally. Every budget here is an acceptance criterion; every statement about macOS 27 behaviour that a spike is named for is a hypothesis until that spike has run.

**What changed in version 3.** The author stated what they use a launcher for: opening apps as fast as possible, calculations, currency conversion, opening specific websites, and possibly asking about a Linux command. File search and clipboard history are not wanted (Maccy already covers the clipboard). Other features are added when the author needs them. The council reviewed versions 1 and 2, not this cut; the architecture decisions it reached are unchanged, and the decisions that only served removed features are withdrawn (section 3).

## 1. What this is

A personal keyboard launcher for macOS: press a hotkey, a panel appears, type a few characters, press Return. The engine is Rust. The window is native AppKit, driven from Rust.

It does five things:

1. **Open or switch to an app**, in the fewest keystrokes and the least time.
2. **Calculate** inline.
3. **Convert currency** inline.
4. **Open a specific website** by keyword.
5. **Answer a question about a shell command** (optional, second milestone).

It deliberately shows nothing else. A query that matches none of these shows an empty list, not a pile of fallbacks.

Why build it when Raycast exists:

- Raycast 2.0 (May 2026) moved to a React UI in a web view plus a long-lived Node process. Raycast's own engineering post puts it at about 350–450 MB in use, against about 40 MB for its native shell alone, and calls those numbers not final. [R1]
- Asyar, a Rust + Tauri launcher, publishes its own benchmark (its harness, one M4 Max, 15 runs): 13.9 ms median to show against Raycast's 22.2 ms, but 419 MB idle against 452 MB. [R2]
- So an existing launcher already opens in about one frame. What none of the web-view launchers offers is a small, quiet process that shows only what you asked for.

The promise: **the app you meant, launched in one or two keystrokes, from a launcher that idles in tens of megabytes and never shows a result type you did not ask for.** Any numeric comparison with another launcher waits for a same-machine measurement.

## 2. What the research found

Three independent reviews were run (Claude, Grok, Astra), then a critique round. Reports: [round1-grok](council/round1-grok.md), [round1-astra](council/round1-astra.md). Critiques: [round2-grok](council/round2-grok.md), [round2-astra](council/round2-astra.md).

| Finding | Consequence |
|---|---|
| Rust launchers have mostly stalled: Loungy (GPUI) archived, RustCast (iced) retired, Gauntlet (iced + Deno + React) ended, Flare (Tauri, Raycast-compatible) archived. The council's inference, not the authors' statement, is that compatibility layers and broad scope made them expensive to keep alive. | Small scope, and the author switches to it personally (G1). |
| The hard part of a launcher on macOS is window behaviour: a non-activating panel that takes keys without activating the app, appears over fullscreen apps, works with input methods, and gives focus back. Reported failures exist for each in GPUI and iced launchers. | The shell is AppKit (D1), and the panel is proven before anything else is built. |
| Vicinae and Floe already pursue Raycast extension compatibility. | Not attempted here. |
| Alfred's script filter (a process prints JSON, the app draws rows) is the cheapest known extension design. | Kept as the designed way to add features later (section 8), not built now. |

## 3. Decisions

| # | Decision | Record |
|---|---|---|
| D1 | **UI: native AppKit via `objc2`** (`NSPanel`, `NSTextField`, `NSTableView`, `NSVisualEffectView`), search engine in pure Rust crates. Escape hatch: a thin Swift shell linking the Rust crates, if `objc2` bindings are the obstacle. | Grok and Astra chose AppKit independently; Claude withdrew GPUI. |
| D2 | **macOS only.** Portability is kept by a pure `core` crate. | Unanimous. |
| D5 | **Ranking authority is the tier order in FR-11. `nucleo-matcher` scores only inside the fuzzy tier.** A typo-tolerant second pass (`neo_frizbee`) only if spike S3 shows it helps. | Dissent (Astra preferred `frizbee` alone) settled by S3. With a catalog of a few hundred apps and links, matcher speed is irrelevant; only ranking quality is tested. |
| D6 | **One resident process, single instance.** No daemon, no helper. The only child processes are the assistant tools started by Ask (FR-34), one at a time, each killed when its answer is no longer wanted. | Unanimous on one process; the Ask exception was added at the author's request. |
| D7 | **Storage: plain files, no database.** Usage counts and cached exchange rates are small tab-separated files replaced atomically; aliases and website keywords live in the TOML configuration. | Changed in version 4: the data is a few hundred numbers, so SQLite added a dependency for nothing. Revisit if a feature needs queries. |
| D13 | **Ranking runs on the main thread.** It takes about 10 µs over the real catalog, so what the model holds is what is on screen, and FR-14 holds by construction for apps and websites. Only work that can block (icons, the rate fetch, Ask) is asynchronous. | Round 3: both members said to keep the search worker off the app-launch path. |
| D8 | **Distribution: a locally signed app on the author's own Mac.** `scripts/install.sh` builds it, installs `/Applications/Rusty Spotlight.app` with an ad hoc signature, and registers a login item (a LaunchAgent that restarts it after a crash but not after Quit); `scripts/uninstall.sh` removes both. Developer ID, notarisation and an updater are added only if someone else is to install it. | Installed on 2026-11-21 at the author's request. |
| D9 | **No telemetry, no accounts, no sync.** The launcher's own network requests are the exchange-rate refresh when a currency query is typed. Ask starts a separate tool that makes its own requests. | Unanimous on the principle; Ask is new. |
| D10 | **Calculator and currency: decided by spike S9.** Units are no longer a requirement, so the simpler option wins unless Numbat is clearly better: a bounded arithmetic evaluator plus a small currency grammar, against Numbat with its currency support. Numbat must pass: cold first evaluation ≤ 150 ms p95, ≤ 10 MB added footprint, evaluation that can be stopped, no network or file access of its own. | `evalexpr` is AGPL and excluded under the default licence (Q4). |
| D12 | **Ask runs installed assistant tools, not an API.** The question is passed to the tool as a single argument, never through a shell; the tool runs in an empty scratch directory with no terminal attached and without the permission-skipping flag the author's own shell alias adds; its output is displayed and copied, never executed. The launcher holds no API key. | Author's decision; not council-reviewed. |

Withdrawn with the features they served: D3 (Spotlight file index), D4 (script extensions in v1; the design is kept in section 8), D11 (clipboard history).

## 4. Goals

- **G1. Daily driver.** The author uses it as their only launcher for five consecutive working days with no fallback to another launcher for apps, calculations, currency or websites.
- **G2. Fast, with numbers.** Every budget in section 6.1 is measured on the author's machine.
- **G3. Cheap when idle.**
- **G4. Correct under asynchrony.** Return runs the item the user last saw selected, or nothing.
- **G5. Uncluttered.** Only the result types in section 1 ever appear.
- **G6. Easy to extend by its owner.** A new feature is a new provider behind one trait, without touching the panel or the ranking.

## 5. Functional requirements

Priority: **M1** daily driver, **M2**, **Later** (designed for, not scheduled).

### 5.1 Panel and summon

| ID | Requirement | Priority |
|---|---|---|
| FR-1 | A global hotkey toggles the panel, registered through the Carbon hotkey path (no Accessibility permission). Set by `hotkey` in the configuration file (modifiers and Space); Option-Space unless set, Command-Space on the author's Mac. Read at start. A failed registration is reported and the status item still opens the panel. Optionally, with Input Monitoring allowed, a listen-only keyboard tap at the hardware end of the chain also catches the shortcut, ahead of other apps' taps that can stall or swallow it; the Carbon event for a press the tap already handled is ignored (added 2026-11-22, author's decision). | M1 |
| FR-2 | The README explains how to free Command-Space from Spotlight. The app never changes system shortcuts itself. | M1 |
| FR-3 | The panel appears on the display and Space where the user is working, including over a fullscreen window and under Stage Manager, without activating the launcher. The frontmost app is unchanged while the panel is open. | M1 |
| FR-4 | The search field accepts typing immediately on summon with no click. The first key after the hotkey reaches the field and not the previous app. | M1 |
| FR-5 | Dismiss with Escape, the hotkey again, or a click outside. Afterwards the previous app has key focus and its caret is where it was. | M1 |
| FR-6 | The panel is created at launch and kept hidden; summon only orders it front. The opening animation is DS-4. | M1 |
| FR-7 | Input methods compose correctly, and confirming a composition with Return never runs a result. Depth of testing depends on Q6. | M1 |
| FR-8 | Menu bar agent with no Dock icon. A status item offers Show, Open configuration and Quit; it can be hidden. | M1 |
| FR-9 | Launch at login via `SMAppService`, off until enabled. | M1 |
| FR-9a | Single instance: a second launch signals the running one to show and exits. | M1 |

### 5.2 Search and ranking

| ID | Requirement | Priority |
|---|---|---|
| FR-10 | One search field. Apps and website keywords are matched synchronously and paint together. The calculator or currency result occupies one reserved row at the top and never displaces a selection the user has moved. Ask runs only on Return after its keyword. | M1 |
| FR-10a | **An empty query shows only the bare field**, exactly as Spotlight opens. Decided by the author on 2026-11-21 after seeing a list on open. A most-used list on an empty query stays available as a configuration option, off by default. | M1 |
| FR-11 | Ranking tiers, in order: (1) exact user alias, (2) exact title, (3) word-prefix and acronym matches, (4) fuzzy subsequence matches. Learned preference reorders within a tier only. | M1 |
| FR-11a | The fuzzy tier applies only to queries of three or more characters. Ties inside a tier break by learned preference, then title prefix before word prefix before acronym, then match score, then shorter title, then title, then identifier. | M1 |
| FR-12 | Learned preference. Stored per item and per (normalised query, item) pair: a count `c` and a timestamp. On a confirmed action: `c ← c · 2^(−days_since_last / H) + 1`. At ranking time: `w = ln(1 + c_item_now) + 2 · ln(1 + c_pair_now)`, capped. `H` defaults to 14 days. Recorded only when an action runs. A (query, item) pair is stored only for queries of two or more characters. Can be reset. | M1 |
| FR-13 | Every edit starts a new generation; results from older generations are discarded. | M1 |
| FR-14 | Return is bound to the **last presented** selection: the item and action on screen in the most recent completed paint. If that item no longer exists, nothing runs. | M1 |
| FR-15 | Matching is case-insensitive and diacritic-insensitive. | M1 |
| FR-18 | **Keyboard.** All keys reach the field's own editor first, so an input method keeps Return, Escape and the arrows while it is composing. Otherwise: Up and Down move the selection while the field keeps focus; Return runs the presented row; Right arrow at the end of the text, or Tab, accepts the completion; Escape dismisses. No Command-number shortcuts. Command-K (M2) shows the selected row's actions inside the same panel, not in a menu. | M1 |
| FR-19 | **One presented record.** The selected row, the inline completion, the Return action and any value to be copied (a calculation result, a converted amount) all come from the same record of what was last drawn. A newer exchange rate is a new paint; Return copies the number that was on screen. | M1 |
| FR-16 | Typo tolerance, only if S3 shows it helps. | M2 |
| FR-17 | User-defined aliases for any app or link (for example `ff` → Firefox). | M1 |

### 5.3 Providers

| ID | Requirement | Priority |
|---|---|---|
| FR-20 | **Apps.** Applications in `/Applications`, `/System/Applications`, `~/Applications` and subfolders. A cached catalog is usable before any scan completes; refresh on directory change. Icons load lazily and never delay typing. | M1 |
| FR-21 | **Running apps.** Selecting a running app switches to it instead of launching a second copy. | M1 |
| FR-20a | **Launch path.** On Return the panel hides and the launch request is issued in the same pass of the run loop; the launcher never waits for the app to finish opening. | M1 |
| FR-22 | **Calculator.** Arithmetic inline with no keyword: `+ − × ÷`, parentheses, powers, percentages (`15% of 2400`, `2400 + 18%`). Return copies the result. Evaluation is bounded; a pathological expression never delays the next keystroke. **A query is treated as a calculation only if** the whole string parses as numbers, operators (`+ - * / × ÷ ^ %`, `of`), parentheses and currency tokens, **and** it contains at least one operator or currency token. A website keyword or the Ask keyword is checked first. Dates written as three number groups, versions and names with digits (`2026-11-21`, `21/11/2026`, `1.2.3`, `2.0`, `1Password`) are not calculations. A two-group form such as `11/21` is treated as a division, because refusing it would also refuse `10/2`. An exact app title or alias keeps the selection; otherwise a calculation starts selected. | M1 |
| FR-32 | **Currency.** `100 usd to inr`, `100 usd inr`, `$100`, and `100 usd` converting to a default currency set in the configuration. Works inside arithmetic (`(120 + 80) eur to inr`). Required currencies: USD, AED, AUD, NPR, INR; any other code the rate source carries also works. Common names and symbols are accepted (`dirham`, `rupees`, `$`, `₹`, `rs`); an ambiguous one (`$`, `rs`, `rupees`) resolves to a configured choice, by default USD and INR, while `US$`, `A$` and `AU$` are explicit. The card always shows both ISO codes. Cross rates go through USD using both legs from the same source, and show the older leg's time. Concurrent refreshes for a pair are merged; a failed fetch backs off; a missing, non-finite or non-positive rate is rejected; with no cached rate and no network the card says the rate is unavailable and copies nothing. Rates are refreshed in the background when a currency query is typed and the cache is older than one hour, never on a timer; the cached rate is shown at once and the row updates if a newer one arrives before Return. Cached rates are used offline. The row shows the rate's time and source. Return copies the number. | M1 |
| FR-23 | **Websites.** User-defined `keyword → URL` in the configuration, with an optional `{query}` slot (`gh rust-lang/rust`, `yt lofi`). A typed URL or bare domain opens directly. Opens in the default browser. | M1 |
| FR-34 | **Ask.** `? question` hands the question to an assistant command-line tool already installed on this Mac and shows what it prints; `?codex question` or `?grok question` picks another tool. Decided by the author on 2026-11-21: any question, not only shell commands, using the installed `claude`, `codex` and `grok` tools instead of an API key. Nothing is sent until Return. The row then reads "Asking…"; a second Return does nothing; Escape, closing the panel or editing the query stops the tool. The answer appears in a card and Return copies all of it; an answer taller than the panel is cut off on screen but copied whole. A tool that fails or gives no answer within 90 s shows an error row, and Return retries. Tools and the default are set in the configuration file. | Built |
| FR-36 | **Units.** `5 km to mi`, `70kg in lb`, `100 f to c`, and a bare amount (`6 ft`) converted to its usual counterpart: length, mass, volume, area, speed, data, temperature. | Built |
| FR-37 | **World time.** `time in dubai`, `sydney time`, `3pm ist to sydney`, `9am dubai` (that time in Dubai, shown here). Cities and abbreviations from a fixed table; offsets, including summer time, from the system's time zone database. Return copies the time. | Built |
| FR-35 | Offline command pages. Dropped: superseded by FR-34 as the author defined it. | — |

### 5.4 Actions and configuration

| ID | Requirement | Priority |
|---|---|---|
| FR-40 | Each result has one primary action on Return. Command-K lists the selected row's other actions in the panel: for apps Show in Finder, Copy Path, and Quit and Force Quit when running; Copy Link for links; copy with the expression for values; Ask another tool for questions and answers. Escape or Command-K goes back. | Built |
| FR-50 | Configuration is one TOML file: hotkey, aliases, website keywords, default currency, Ask endpoint. A malformed edit keeps the last good configuration and reports the error in the panel. | M1 |
| FR-54 | Appearance follows the system light/dark setting, uses system materials, and honours Reduce Transparency and Increase Contrast. | M1 |
| FR-51 | A settings window. | Later |

### 5.5 Visual design

Decided by the author on 2026-11-21: **the panel matches Spotlight on macOS 27**, built from the same public system pieces. A rough HTML sketch of the six states is in [design/panel-mockup.html](design/panel-mockup.html); its sizes are guesses and are superseded by the measurements below once taken.

| ID | Requirement | Priority |
|---|---|---|
| DS-1 | System font, SF Symbols for non-app rows, real app icons, system label colours, the user's accent colour. No custom palette, no theme engine. | M1 |
| DS-2 | Background is the system glass material Spotlight uses (`NSGlassEffectView`), if the `objc2` bindings expose it and it fits the 40 MB empty-panel budget; both are checked in S1. Otherwise `NSVisualEffectView`, which is close but not identical. | M1 |
| DS-3 | Sizes follow Spotlight as measured on the author's Mac (table below). | M1 |
| DS-3a | The completion is a separate, non-editable overlay placed after the typed text, never part of the field's string, and is not an accessibility element. It is hidden during input-method composition and when the caret is not at the end. When the selected title does not continue what was typed (an alias, an acronym), it shows ` — Title` instead of a tail. The query field shows an inline completion after the caret, as Spotlight does: the rest of the top result's name and its action (`te` → `rminal — Open`), or `= result` for a calculation or conversion, with the result's icon at the trailing edge of the field. | M1 |
| DS-3b | The selected row is a neutral translucent fill, not the accent colour. | M1 |
| DS-4 | **Motion, as Spotlight.** Measured by recording Spotlight at 60 fps over a plain black card on macOS 27.2 and tracking the field's edges in every frame; the launcher was then recorded and measured the same way. No published article gave Apple's spring values, so the recording is the source. (1) **Opening:** Spotlight's field is first visible at about 110% of its width and 96% of its height, reaches its resting shape in about 125 ms with 0.2% overshoot, and fades in over about 75 ms. Launcher, measured: 108% and 96%, same duration, same overshoot. (2) **Resize spring:** about 370 ms after opening, Spotlight's field retracts to make room for four round buttons that bud off its right end. Over 499 px of travel it overshoots by 8.6% at about 240 ms, rebounds slightly past the target at about 475 ms and rests by about 650 ms: a damping ratio of about 0.62 at about 17 rad/s. This is the visibly springy motion. The launcher has no buttons (Q17), so it uses that same spring where its own panel changes size: growing from the field to a result list and between list lengths. Launcher, measured: 8.4% overshoot at about 210 ms, rebound at about 425 ms. All motion is visual only: the field has focus and accepts keys from the first frame. The window is a fixed size, larger than the panel, so overshoot is not clipped. Skipped under Reduce Motion. No hide animation. | M1 |
| DS-7 | The drop shadow is a rounded shadow on a layer behind the glass, not the window shadow (which is rectangular and is turned off). With Reduce Transparency the glass is replaced by an opaque system background; with Increase Contrast the selection gains an outline. | M1 |
| DS-5 | Deliberate departures from Spotlight: only this project's result types appear; the selected row names its Return action; a running app shows a dot and switches instead of opening; the bare field is the full 640 pt wide because there are no filter buttons beside it. The system's own text-suggestion popups are turned off for the field. | M1 |
| DS-6 | Calculator and currency results use Spotlight's result card: the expression in small secondary text, the result beneath, a copy button at the right; currency adds the rate, source and time. | M1 |

**Spotlight measurements.** Taken on 2026-11-21 from screenshots of Spotlight on macOS 27.2, dark appearance, at 2× scale, by reading pixel positions; each is good to about ±1 pt. Light appearance was not captured and is derived from the system colours.

| Element | Measured |
|---|---|
| Panel width | 640 pt, constant between the empty and expanded states |
| Position | horizontally centred; top edge about 28% down the screen |
| Empty state | a 56 pt high, fully rounded field (Spotlight's is 383 pt wide followed by four 54 pt round buttons; this project has no buttons, so the field takes the full 640 pt) |
| Corner radius, expanded panel | about 28 pt |
| Field | 56 pt high; search glyph 23 pt, 22 pt from the left edge; query text about 25 pt, starting 61 pt from the left edge; trailing icon 26 pt |
| Separator | 1 px, inset 19 pt each side |
| Rows | 56 pt pitch; app icon 36 pt, 16 pt from the edge; title 17 pt medium, 18 pt after the icon; subtitle 15 pt secondary |
| Selection | neutral translucent fill, inset 9 pt from the panel edge, radius about 18 pt |
| Row hint | 11 pt secondary text and a small key cap, trailing |
| Result card | inset 18 pt, radius about 20 pt, translucent fill with a 1 px lighter border; expression 13 pt secondary, result 17 pt; 26 pt round copy button |
| Visible height | about 466 pt before scrolling, roughly seven rows |

Also observed: Spotlight itself credits Yahoo Finance on its currency card, the same source this project uses.

**Side-by-side corrections (2026-11-21).** Spotlight and the launcher were captured in the same state over the same backgrounds (the desktop, and a white/grey/colour test card) and compared zoomed. Corrections made, each then re-checked against a new capture:

| Element | Was | Now, matching Spotlight |
|---|---|---|
| Glass | regular `NSGlassEffectView` | unchanged: the regular style matches Spotlight's tone and blur over every band of the test card; the clear style does not |
| Drop shadow | none | soft shadow, about 26 pt blur, offset 12 pt down, drawn only outside the panel's outline so it does not darken the glass |
| Search icon | 19 pt, light | about 24 pt, medium weight |
| Typed text and row titles | system label colour (slightly translucent) | full-strength white in dark appearance, black in light |
| Row title weight | medium | regular, 17 pt |
| Completion | misaligned with the typed text, dimmer | same baseline; brighter text on a slightly lighter chip |
| App icons | 29 pt visible | 36 pt visible (a 44 pt view, since icon artwork fills about 80% of its canvas) |
| Row content position | about 6 pt too far right | icon 17 pt and title 70.5 pt from the panel edge |
| Return hint | text and an arrow | action name and a rounded key cap |
| Field | nothing at the trailing edge | the selected result's icon |

Not compared: light appearance, Increase Contrast, Reduce Transparency, and the calculator card.

**Round 3 record.** Both members confirmed the dark geometry is close enough to build from and that light appearance is unverified. Disagreements and how they were settled:

| Topic | Grok | Astra | Settled |
|---|---|---|---|
| Hiding when the panel loses key status | Remove it; use a global mouse-down monitor | Keep it as a signal, but centralised | Kept, decided one run-loop turn later and only if the panel is still not key. Replaced by a mouse monitor if the manual checks show a menu or input palette dismissing the panel. |
| Completion when the title does not continue the query | Show nothing | Show a tail only for a literal prefix | ` — Title` is shown, because that is what Spotlight was observed to do. |
| The grey chip behind the completion | Not in the measurement table | — | Kept: it is visible in the Spotlight captures. |
| Starting window level | Whatever is logged, re-tested | Floating, then raise | Pop-up level, now actually applied (Grok found the earlier build ran at floating level because of call order). The fullscreen checks decide. |
| Tab | Accepts the selected title | Leave for key-view traversal | Accepts the completion; the panel has one focusable control. |

## 6. Non-functional requirements

### 6.1 Performance budgets

Measured on the author's Apple Silicon Mac, release build. "Hidden" means the panel has been hidden for at least 60 seconds. Two timestamps are reported separately: **input-ready** (the field accepts a key event) and **pixels** (the frame is on screen, checked by timestamped screen capture on a subset of runs). At 60 Hz one frame is 16.7 ms.

| ID | Measure | Gate |
|---|---|---|
| NFR-1 | Hotkey to panel pixels and input-ready, warm, over 1,000 summons | p50 ≤ 16 ms, p95 ≤ 33 ms. |
| NFR-2 | First key after the hotkey | In 1,000 of 1,000 automated summons: the hotkey registration returned success, the sentinel character arrives in the field as a key event, and the previously frontmost app's document does not contain it. |
| NFR-3 | Keystroke to results on screen | p95 ≤ 33 ms to pixels at 60 Hz; within that, matching ≤ 1 ms over the real catalog and main-thread update ≤ 3 ms. |
| NFR-3a | Return to launch request issued and panel hidden | p95 ≤ 16 ms. The time the target app then takes to appear is reported for orientation and is not the launcher's to gate. |
| NFR-5 | Process start to hotkey working | p95 ≤ 400 ms with a saved catalog. |
| NFR-6 | Physical footprint, hidden | Empty panel (spike S2): ≤ 40 MB. Full app: target ≤ 50 MB, gate ≤ 80 MB. |
| NFR-7 | CPU while hidden | < 0.2% of one core over five minutes; no display link; no periodic timers. |
| NFR-9 | Ranking quality on 50 app queries from the author's real use, frozen before the test week | After a week of use: ≥ 90% top-1 and ≥ 98% top-3. |
| NFR-10 | Wrong-action executions | Zero in the race suite, which delays painting independently of result delivery and asserts the executed item against what was visible. |
| NFR-11 | Calculator and currency correctness | A fixture of 100 expressions (precedence, percentages, large and small magnitudes, currency forms) all match expected results; text that is not an expression (an app name containing digits, a date) never produces a calculator row. |

### 6.2 Reliability

- NFR-21. Works after sleep/wake, display connect/disconnect, Space changes and Stage Manager.
- NFR-22. A corrupt database is detected at start; the app rebuilds the catalog and keeps running.
- NFR-24. With the network off: apps, calculator, websites (row built and handed to the browser) and currency from cached rates all work. A rate older than three days is marked stale in the row.

### 6.3 Privacy and security

- NFR-30. No permission is required by any M1 feature: no Accessibility, no Full Disk Access. Input Monitoring is asked for once, for the shortcut tap in FR-1; refused, the Carbon hotkey works alone.
- NFR-33. No telemetry, no crash upload, no account. A local log only.
- NFR-34. Website keywords URL-encode the query; only `http` and `https` URLs are opened.
- NFR-37. Ask: the tool receives a fixed instruction and the typed question, nothing about the machine, the running apps or the clipboard. What the tool itself then sends to its service, and what it can read or do, is governed by that tool's own configuration and sign-in, not by this launcher; the launcher only starts it in an empty directory and without a terminal. The answer is treated as text: displayed and copied, never executed.

### 6.4 Accessibility and platform

- NFR-40. VoiceOver announces the search field and the selected row. Full keyboard operation.
- NFR-50. Apple Silicon, macOS 27 (the author's machine).
- NFR-51. `core` and `providers` (with test doubles) compile and pass tests on Linux.
- NFR-52. All `unsafe` Objective-C interop is confined to the macOS shell crate.

## 7. Technology choices

Versions checked against crates.io on 2026-11-21 by at least two council members, except where marked.

| Area | Choice | Version | Note |
|---|---|---|---|
| Language | Rust | 1.100 | pinned in `rust-toolchain.toml` |
| AppKit bindings | `objc2`, `objc2-app-kit`, `objc2-foundation` | 0.6.5 / 0.3.2 | |
| Global hotkey | `global-hotkey` | 0.10.0 | Carbon `RegisterEventHotKey`; main thread only |
| Fuzzy matching | `nucleo-matcher` | 0.3.1 | MPL-2.0 |
| Typo pass (conditional) | `neo_frizbee` | 0.13.3 | MIT |
| Storage | plain tab-separated files | — | no dependency (D7) |
| Calculator (candidate) | `numbat` | 1.24.0 | MIT/Apache; subject to D10. Whether its currency module can take rates from our cache instead of fetching them itself is unverified until S9 |
| HTTP (rates, Ask) | `NSURLSession` through `objc2`, behind a trait | system | No extra TLS stack in the binary. Not council-reviewed |
| Exchange rates, primary | Yahoo Finance chart endpoint, one request per pair against USD (`query1.finance.yahoo.com/v8/finance/chart/USDINR=X`, reading `meta.regularMarketPrice` and `meta.regularMarketTime`) | checked 2026-11-21 | The author's choice. Returned AED, AUD, NPR and INR without a key or cookie when checked through a fetch proxy; not yet checked from this Mac. Intraday market rates. **Unofficial**: undocumented, no published terms permitting this use, and the sibling `v7/finance/quote` endpoint already answers 401, so it can stop working without notice. Acceptable for a personal tool; reconsider if the app is ever distributed (Q15). Other pairs are derived through USD |
| Exchange rates, fallback | `open.er-api.com/v6/latest/USD` (ExchangeRate-API open endpoint), used when Yahoo fails | checked 2026-11-21 | Keyless; covers USD, AED, AUD, NPR and INR; updates once a day; rate-limited (HTTP 429, lifts after 20 minutes); requires the attribution "Rates By Exchange Rate API"; caching allowed, redistribution not. Its full terms page has not been read yet (S9). Frankfurter (ECB) is ruled out: it has no AED or NPR. On the day checked the sources differed by up to about 0.8% (USD→INR 92.67 on Yahoo, 93.37 here), so results are indicative market rates, not a bank's rate |
| Config | `toml` + `serde` | current | |
| Not used | Tauri, web views, Node, Deno, GPUI, iced, an async runtime, SQLite, `evalexpr` (AGPL), Sparkle (until D8's condition) | | |

## 8. Adding features later

The author adds features as the need appears. Two routes, in order of preference:

1. **A new built-in provider.** Each provider implements one trait (take a query and a generation, return rows with stable identifiers and one action). The panel, ranking and Return handling do not change. This is the default route for a personal tool.
2. **Script commands**, when a feature is easier in a shell script than in Rust: an executable in a scripts directory with a small manifest, invoked by keyword, printing rows in Alfred's script-filter shape (`{"items":[{"uid","title","subtitle","arg"}]}`). Out of process, with a deadline and output limits; nothing runs merely because the user typed. Designed in version 2 of this document; built when the first such feature is wanted.

Every addition is keyword-scoped or opt-in, so the default result list stays as short as it is now (G5).

## 9. Out of scope

| Not built | Reason |
|---|---|
| File search, content search, own file index | Not wanted; the author finds it clutter. |
| Clipboard history | Maccy already does it for the author. |
| Emoji picker, snippets, system commands, window switcher | Not in the author's stated use. Each is a later provider if wanted. |
| Chat pane, conversation history, AI over files or screen | Ask is one question, one answer, in rows. |
| Raycast extension compatibility, extension store | Needs Node and a React reconciler; other projects are well ahead. |
| Settings window, CLI, URL scheme, updater, notarised DMG | Only needed once someone else uses it. |
| Linux and Windows | The author uses macOS. |
| Accounts, sync, telemetry | D9. |

## 10. Open questions

Each has a default the plan assumes until answered.

| # | Question | Default |
|---|---|---|
| Q2 | Should it take over Command-Space? | **Answered: yes.** `hotkey = "cmd+space"` in the configuration file. Spotlight's own shortcut was already switched off in System Settings on this Mac; the launcher did not change it. |
| Q4 | Licence, if the source is ever published? | MIT OR Apache-2.0, so no AGPL dependencies. |
| Q6 | Languages you type daily? | English only; input-method testing is then a short smoke check. If you type Hindi or another non-Latin script in the launcher, the full composition tests run in S4. |
| Q12 | What should Ask do? | **Answered:** any natural-language question, answered by an installed assistant tool. |
| Q13 | Which model endpoint and key for Ask? | **Answered:** none; the installed `claude`, `codex` and `grok` tools, which use their own sign-ins. |
| Q14 | Default target currency? (Answered in part: the currencies are USD, AED, AUD, NPR and INR.) | INR, as an assumption; one line in the configuration. |
| Q17 | Spotlight's springiest motion is its four buttons budding off the field. Should the launcher have buttons there (for example Apps, Calculator, Websites, Ask as mode filters), a stronger bounce on the field alone, or stay as it is? | Stay as it is. |
| Q16 | On an empty query: most-used apps, or a bare field? | **Answered: bare field**, as Spotlight. |
| Q15 | Will anyone else install this? | No. If yes, D8's Developer ID, notarisation and updater return to the plan. |

## Sources

- [R1] Raycast, "A Technical Deep Dive Into the New Raycast", 2026-05-14 (figures described by the authors as not final): https://www.raycast.com/blog/a-technical-deep-dive-into-the-new-raycast
- [R2] Asyar's own README benchmark and harness (vendor-run, one machine, 15 summons; memory is summed resident size): https://github.com/Xoshbin/asyar#benchmarks
- Loungy (archived): https://github.com/MatthiasGrandl/Loungy · Gauntlet (ended): https://github.com/project-gauntlet/gauntlet · Flare (archived): https://github.com/ByteAtATime/flare · RustCast (retired): https://github.com/MystikoLab/rustcast
- Alfred script filter JSON: https://www.alfredapp.com/help/workflows/inputs/script-filter/json/
- Non-activating panel: https://developer.apple.com/documentation/appkit/nswindow/stylemask-swift.struct/nonactivatingpanel · key handling: https://developer.apple.com/documentation/appkit/nspanel/becomeskeyonlyifneeded
- Overlay above fullscreen, Apple forum thread: https://developer.apple.com/forums/thread/826308
- Carbon hotkey modifier restriction and its reversal, Apple forum thread: https://developer.apple.com/forums/thread/763878
- `global-hotkey`: https://docs.rs/global-hotkey/0.10.0/global_hotkey/
- `nucleo`: https://github.com/helix-editor/nucleo · `frizbee`: https://github.com/saghen/frizbee · `evalexpr` licensing: https://docs.rs/crate/evalexpr/13.1.0#licensing
- The council reports carry the full source lists and mark which claims each member verified on the day.
