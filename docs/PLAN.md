# Plan: building the launcher

Status: version 4, 2026-11-21. Updated with progress and council round 3.
Read [REQUIREMENTS.md](REQUIREMENTS.md) first; identifiers (FR-, NFR-, D-, Q-) refer to it.

## 0. Where things stand

Built and running: the workspace; the panel with the system glass, hotkey and status item; app discovery; the tiered ranking with learned preference; the results table with icons, selection, inline completion, keyboard handling; launch and switch; usage saved to disk.

Measured so far, informally (a handful of summons driven by synthesized keys, one app in front, in-process timers; none of this is the benchmark of section 4):

| What | Result |
|---|---|
| Option-Space registers on macOS 27.2 | yes |
| Typing reaches the field with no click; frontmost app unchanged | yes, on an ordinary Space |
| `NSGlassEffectView` available through `objc2-app-kit` 0.3.2 | yes |
| Footprint while hidden | 19 MB at start, 22–26 MB after use (gate 40 MB) |
| App scan | 108 apps, about 10 ms warm, 72 ms cold |
| Ranking | about 10 µs per query |
| Summon, in-process | 3.5–7 ms for the bare panel; 34 ms once with a catalog rescan on the summon path, since moved after the panel is shown and not re-measured |

Corrected by round 3: the earlier spike ran at floating level, not pop-up, because `setFloatingPanel` was called after `setLevel`. Fixed and logged; the observations above were made at floating level.

Added since: the opening and resize motion measured from Spotlight; the visual corrections from side-by-side captures; the configuration file (`~/.config/rusty-spotlight/config.toml`) with aliases and website keywords; the calculator; currency conversion with live Yahoo Finance rates and the ExchangeRate-API fallback; standard editing shortcuts in the field.

Checked in the running app by capture: `2400 + 18%` shows 2,832; `100 usd to inr` shows 9,267.00 INR at 92.67 (the figure Spotlight showed the same day); `250 aed to npr` shows 10,107.02 NPR through the dollar; `$40` converts to the default currency; `gh rust-lang/rust` offers github.com/rust-lang/rust; `1Password` produces no calculator card. 20 unit tests pass (17 in the engine, 3 for configuration).

Ask is built: `? question` runs the installed `claude` tool (or `?codex`, `?grok`). Checked end to end in the running app with one question to Claude: the row, the waiting state, the answer card about 16 s later, and Return copying the answer. Each of the three tools was also run once from the command line the way the launcher runs it and answered in 9–12 s. Not checked in the panel: Codex and Grok, cancelling a question in flight, an answer longer than the panel, and a failing tool. 24 unit tests pass.

A full pass over the running app then checked, by driving it and reading the clipboard, the process list and captures: Return copies a calculation and a conversion; Return launches an app and records the launch; a running app reads "Switch"; Down, Up and Tab; a website keyword opens in the browser; Ask with Claude, Grok and Codex; cancelling a question by Escape and by editing (the tool's process is killed both ways); a missing tool and a silently failing tool show an error row; a broken configuration file keeps the last good settings.

Four problems from that pass were fixed and re-checked: a panel summoned within a few seconds of launching an app no longer disappears when that app finishes starting; an answer too long for the panel says so in its heading and is still copied whole; a broken configuration file is reported in the panel on open, and Return opens the file; unexpected hides are logged under `RS_DEBUG`. One blank panel seen once after a failed Ask could not be reproduced in three repeats.

Daily use found one more on 2026-11-22: opened over T3 Code, the panel hid within a second because T3 took the keyboard back by itself, with no click and no app switch. Losing the keyboard now hides the panel only when the user left: a mouse button is down, or a different app is in front 0.12 s later. If the app it opened over takes the keyboard, the panel takes it back, at most four times per summon. Every hotkey press, show, keep and hide is logged to `~/Library/Logs/rusty-spotlight/launcher.log`. Checked: switching to another app still hides it. Not reproducible from outside T3, so it is checked by use.

A second report, Command-Space doing nothing at all, mostly with T3 in front: the log shows the key never reached the launcher. T3 binds no Command-Space. Wispr Flow holds an active keyboard tap that sees every key before the hotkey is matched, and it has recorded worst delays of 0.7 s and 2.7 s. The launcher now checks every 2 s and logs when secure keyboard input turns on (with the app) and when another app's keyboard tap sets a new worst delay or is switched off, so the next miss can be matched to a cause.

Menu bar and settings (2026-11-22): the menu bar icon is the system magnifying-glass symbol drawn as a template, and its menu is Open (showing the shortcut), Settings… and Quit. A Settings window holds "Show icon in menu bar", saved as `menu_bar_icon` in config.toml (a hand edit applies on the next summon). With the icon hidden, Settings is reached by typing "settings" in the launcher (the app's own entry is listed as "Rusty Spotlight Settings" and no longer starts a second copy) or by opening the app again in Finder. Checked: the window and icon drawn off screen with `--snapshot-settings DIR`; the configuration line edit by a unit test. Not yet checked by eye: the live icon, hiding and showing it, and the reopen path.

Shortcut tap, units, world time and Command-K (2026-11-22). The author approved Input Monitoring for the shortcut: a listen-only tap at the HID end of the chain, inserted at the head, sees Command-Space before BetterTouchTool's and Wispr Flow's taps and toggles the panel; the Carbon event for the same press is ignored for up to 5 s, and a press the Carbon hotkey never delivered is logged. The app is now signed ad hoc with the designated requirement `identifier "local.rusty-spotlight"`, so the permission survives a reinstall; checked by reinstalling after the grant. Settings shows whether the tap is on, with a button to Input Monitoring. Units (FR-36) and world time (FR-37) are in the core crate with 7 new tests; a shell test checks every zone in the table against the system database and Sydney's summer time. Command-K (FR-40) lists the selected row's actions in place of the results. Checked in the running app with synthetic keys: the tap caught every press and no press toggled twice; `time in dubai`, `3pm ist to sydney`, `6 ft` and Command-K on a running and a closed app drawn correctly. Not checked: Quit and Force Quit, Ask another tool, and whether real missed presses stop, which needs daily use.

Warm summon measured 12–20 ms in-process across 19 summons (31 ms on the first after launch); footprint 32 MB after the pass.

Still not checked: the fallback rate source; behaviour with the network off.

**Not yet verified by anyone:** the panel over a fullscreen app, on another Space, on a second display, with Stage Manager, after sleep and wake; click-outside dismissal; caret restoration; Return, arrows and row clicks in the new table; input methods; VoiceOver; light appearance. These are the author's manual checks and they gate everything after app launch.

**Order of work from here:** the author's manual checks and daily use → fix what they find → the benchmark tool and the M1 gates (summon is measured at about 28 ms in-process, over the 16 ms target, and not yet investigated) → the remaining Ask checks. The matcher comparison (S3's second matcher) and Numbat (S9) are off the critical path: one strict matcher and a small arithmetic evaluator ship first.

## 1. Approach

1. **Prove the window before writing features.** The panel's focus behaviour is the product. Nothing else is built until a bare panel passes its tests on this Mac.
2. **Launch apps with it from the first week of M1.** Everything after that is added to a tool already in daily use.
3. **A green measurement must mean the product works.** The benchmark's pass conditions cannot be met by a shortcut (section 4).
4. **Nothing is added that was not asked for.** New features arrive one provider at a time, when the author wants them.

Scope is apps, calculator, currency and websites, then Ask. Sizing assumes one developer full time with coding agents. The numbers are estimates, not measurements; a milestone ends when its exit criteria are true.

| Milestone | Effort (estimate) | Ends when |
|---|---|---|
| M0 Spikes | about 2 weeks | The bare panel works on this Mac; matcher and calculator choices are settled by measurement |
| M1 Daily driver | about 3–4 weeks (last week is use) | Five days as your only launcher |
| M2 Ask and polish | about 1–2 weeks | Ask works, or is dropped by your choice |

## 2. Architecture

### 2.1 Process and threads

One resident, single-instance process, started at login, with no Dock icon and no child processes.

```
                       ┌──────────────────────── launcher process ────────────────────────┐
  hotkey (Carbon) ───▶ │ main thread: AppKit run loop, panel, text field, table,          │
                       │              apply results, record what was last presented       │
                       │        ▲ results (generation-tagged)       │ query + generation  │
                       │        │                                   ▼                     │
                       │ (ranking and calculation run on the main thread: ~10 µs)        │
                       │                                                                  │
                       │ background:    app scan, icon decode, rate refresh on use,       │
                       │                Ask request (only after Return)                   │
                       └──────────────────────────────────────────────────────────────────┘
```

- **Main thread** owns every AppKit object. It applies results and records the identity of the row and action it last painted as selected. Return dispatches that recorded pair (FR-14). It never matches, reads the database, decodes icons or waits on the network.
- **Ranking** runs on the main thread against the in-memory catalog (D13), so there is no search worker and no stale result to discard for apps and websites.
- **Background work** uses plain threads with channels. No async runtime.

### 2.2 Workspace

```
crates/
  core/          item and action identity, query parsing, tiers, matching, frecency formula,
                 generation and presented-selection rules, the provider trait.
                 No OS types, no database types.
  providers/     apps, calculator, currency, websites, ask. Each reaches the OS or the
                 network through a trait the shell implements.
  shell-macos/   the binary: AppKit lifecycle, NSPanel, hotkey, status item, login item,
                 app discovery, NSURLSession, Keychain. All unsafe FFI.
tools/
  bench/         summon, typing and memory benchmark harness with an external observer.
  fixtures/      frozen app-query set, calculator/currency expression fixture.
docs/
  bench/         committed benchmark results with OS build and hardware.
```

`core` and `providers` (with test doubles) build and test on Linux. Only `shell-macos` links `objc2`.

### 2.3 Data

| What | Where | Format |
|---|---|---|
| Configuration: hotkey, aliases, website keywords, default currency, Ask endpoint | `~/.config/<app>/config.toml` | TOML, user-edited, last good kept |
| Usage counts, exchange rates | `~/Library/Application Support/<app>/usage.tsv`, `rates.tsv` | tab-separated text, replaced atomically |
| Ask API key | macOS Keychain | |
| Icon cache | `~/Library/Caches/<app>/` | disposable |
| Logs | `~/Library/Logs/<app>/` | local only |

### 2.4 The panel: what is known and what S1 must find out

Known from Apple's documentation: an `NSPanel` with the non-activating style can become key without activating its app, provided `canBecomeKey` is true and `canBecomeMain` is false.

Not known, and the two round-one reports disagree on the starting point:

| Question | Candidates to test in S1 |
|---|---|
| Collection behaviour | (a) `canJoinAllSpaces` + `fullScreenAuxiliary` + `stationary` + `ignoresCycle`; (b) `canJoinAllApplications`; (c) `moveToActiveSpace`. These are alternatives, not a mix. |
| Window level | Test floating, pop-up and higher levels over a fullscreen app and record the lowest that works. |
| Show and dismiss | `orderFrontRegardless` + make key; `orderOut` on dismiss. Confirm the frontmost app is unchanged throughout. |
| Outside click | Hide when the panel resigns key, or a global mouse-down monitor. Confirm neither needs a permission. |
| Hotkey | Whether Option-Space registers on macOS 27, and the fallback default if not. |

Content: `NSVisualEffectView` background, `NSTextField` input, view-based `NSTableView` with fixed row height. Position on the display under the pointer.

## 3. Milestones

### M0 — Spikes (about 2 weeks)

| Spike | What is done | Pass condition | If it fails |
|---|---|---|---|
| **S1. Panel and hotkey** (days 1–4) | A binary that is only the panel, a text field and stub rows. Work through the table in 2.4. Manual matrix: Terminal, a browser, an editor, a fullscreen video, a second Space, a second display, Stage Manager, after sleep/wake, rapid toggle, click outside, an already-taken chord. | For every row: panel appears where you are; typing works with no click; the frontmost app is unchanged; Escape returns the caret; no Dock icon; no permission prompt. The chosen default chord registers on macOS 27. | Fix panel semantics first. If `objc2` is the obstacle, take the Swift-shell escape hatch (D1). |
| **S4. Input methods and VoiceOver** (one day, on the S1 binary) | English-only (Q6 default): confirm dead keys and the emoji picker work in the field, and VoiceOver reads the field and the selected row. If you type a non-Latin script in the launcher: composition on an ordinary Space and over a fullscreen Space. | Composition confirms without running a result; VoiceOver reads the field and row. | If the candidate window cannot appear over fullscreen without activation, the panel activates the app only while marked text exists; record the limit and move on. |
| **S2. Bench and empty-panel budgets** (days 4–7) | Build `tools/bench` as specified in section 4. Baseline your current launcher with it. Run the 1,000-summon gate on the S1 binary. | NFR-2 holds as written. NFR-1 and NFR-7 hold. Empty-panel footprint ≤ 40 MB. | Find the cause before adding anything. Over 40 MB on an empty panel reopens D1. |
| **S3. Ranking** (week 2) | Real app catalog. FR-11's tiers with `nucleo-matcher` in the fuzzy tier; `neo_frizbee` behind the same trait. A frozen set of 50 app queries from your own use, including the typos you actually make. Race suite: delay painting independently of result delivery, inject late results before Return. | Tier order asserted by golden tests. Zero wrong-action executions, asserted against what was visible (NFR-10). The typo pass is adopted only if it raises top-3 on your typo queries without lowering the rest; ties go to the simpler setup. | Ship M1 with strict matching only. |
| **S5. Apps and launch** (week 2) | App discovery, catalog cache, lazy icons, launch and switch. | Cached catalog usable before the first scan finishes; cold icon cache never delays typing; NFR-3a holds; a running app is switched to, not relaunched. | — |
| **S9. Calculator and currency** (one day, week 2) | Two candidates behind one trait: a bounded arithmetic evaluator with a small currency grammar, and Numbat. Fetch the four USD pairs from Yahoo Finance's chart endpoint from this Mac through `NSURLSession` (it answered without a key through a proxy on 2026-11-21; whether it needs a particular User-Agent or rate-limits here is unknown). Wire `open.er-api.com` as the fallback behind the same trait and read its terms. | The 100-expression fixture passes (NFR-11) for the chosen candidate; app names with digits never produce a calculator row. Numbat is chosen only if it passes D10's thresholds **and** can take rates from our cache; otherwise the simple evaluator ships. A rate source is confirmed reachable and its licence or terms recorded. | Yahoo refuses requests from this Mac: the fallback becomes the primary. |

**M0 exit.** S1, S4 and S2 pass: this closes the windowing stack unless a budget later fails. A short decision note in the README records the outcomes of S3 and S9 and the measured empty-panel numbers.

### M1 — Daily driver (about 3–4 weeks)

Scope: panel and summon (FR-1 to FR-9a), search and ranking (FR-10 to FR-15, FR-17), apps and running apps, calculator, currency, websites, TOML configuration, appearance.

Build order, each step leaving a usable app:

1. `core`: items, actions, tiers, frecency (FR-12), generations, presented-selection rule, with S3's golden and race tests as the regression suite.
2. Apps: launch and switch, usage recording, most-used apps on an empty query (FR-10a). **From here you launch apps with it every day.**
3. TOML configuration with last-good fallback; aliases; single instance.
4. Websites: keywords with `{query}`, direct URLs.
5. Calculator and currency per S9; background rate fetch and cache; stale marking.
6. Status item, login item, README with the Command-Space instructions.
7. **Final week: use, not building.** Fix only what blocks the switch. Quit your current launcher for these five days.

**M1 exit.**
- Five consecutive working days as your only launcher, with no fallback for apps, calculations, currency or websites (G1).
- Gates green on the real catalog: NFR-1, NFR-2, NFR-3, NFR-3a, NFR-5, NFR-6, NFR-7, NFR-9, NFR-10, NFR-11.
- A written list of anything you missed. That list, not this document, decides what is built next.

### M2 — Ask and polish (about 1–2 weeks)

Starts only after Q12 and Q13 are answered; if you decide you do not want Ask, M2 is just the polish items.

- **Ask** (FR-34): `?` keyword, request only on Return, deadline, cancel, answer as rows, key in the Keychain. Checks: with the network off or no key, the row says so and nothing hangs; a stubbed slow endpoint is cancelled by Escape and by typing; a captured request contains only the question and the fixed instruction (NFR-37); the answer is never executed.
- **Offline command pages** (FR-35), if Q12 keeps them: bundled tldr pages, with their licence recorded and the added download and footprint measured against NFR-6.
- **Polish:** secondary actions for apps (FR-40), typo pass if S3 admitted it, whatever M1's list put first.

**M2 exit.** Ask answers a question end to end and passes its checks, or is explicitly dropped; all M1 gates still green with Ask enabled and idle.

### After M2

Features are added when you want them, by the two routes in section 8 of the requirements: a new provider, or script commands the first time a feature is easier as a script. Each addition is keyword-scoped or opt-in, and the M1 gates are re-run after it. If someone else is to install the app (Q15), Developer ID signing, notarisation and an updater come back as their own piece of work.

## 4. Measurement

- **Two timestamps, never merged.** Input-ready (the field received a key event) and pixels (the frame is on screen, by timestamped screen capture on a subset of runs). A window existing in the window list counts as neither.
- **`tools/bench`.** An external process posts the real hotkey chord and then a sentinel key as system key events. A summon passes only if the hotkey registration reported success, the sentinel arrives in the field through the normal event path, and a scratch document in the previously frontmost app does not contain it. The app must not write the sentinel itself. Reports p50/p95/p99 over 1,000 warm summons and 100 warm process starts, physical footprint, and idle wakeups over five quiet minutes.
- **Launch.** Return to launch-request-issued and panel-hidden (NFR-3a) is timed inside the app; time until the target app's window appears is recorded alongside, for orientation.
- **Baseline.** The same observer, same machine, against your current launcher, version recorded. No comparative claim is made without this.
- **CI (Linux).** `core` and `providers` tests: tier-order goldens, generation and presented-selection races, the calculator and currency fixture, configuration parsing.
- **Local gate.** The bench runs before each milestone exit; results are committed to `docs/bench/` with OS build and hardware.

## 5. Risks

| # | Risk | Retired by |
|---|---|---|
| 1 | The panel steals focus, drops the first key, or fails over fullscreen or Stage Manager. | S1, before anything else. |
| 2 | A benchmark goes green while the product is broken. | Section 4's pass conditions; the external observer. |
| 3 | Return runs something other than what was on screen. | FR-14; S3's race suite kept as a permanent test. |
| 4 | The calculator row appears for things that are not calculations, which is the clutter this project exists to avoid. | NFR-11's negative cases in the fixture. |
| 5 | Yahoo Finance's endpoint is unofficial and starts refusing requests, as its quote endpoint already does. | Automatic fallback to a second source behind the same trait; at most one refresh an hour, only when you convert; cached rates keep working with a stale mark. |
| 6 | Idle memory or CPU creeps over budget. | 40 MB empty-panel gate in S2; the bench at each exit. |
| 7 | Ask leaks more than the question, or a wrong command is trusted. | NFR-37's captured-request check; copy only, never run; the explanation line is shown with the command. |
| 8 | `objc2` friction slows the shell. | S1; the Swift-shell escape hatch. |
| 9 | Features accumulate and the result list becomes what you left Raycast for. | G5; every addition keyword-scoped or opt-in. |
| 10 | Option-Space does not register on macOS 27, or input methods fail over fullscreen. | S1 and S4, each with a written fallback. |

**Stop conditions.** Work stops until the cause is removed if: idle footprint exceeds 80 MB; a second UI toolkit, a web view, or a Node or Deno dependency appears; a result type shows up for a plain query that the author did not ask for.

## 6. First week

1. Answer Q6 (languages) and Q14 (default target currency), or accept the defaults. Q12 and Q13 can wait until M2.
2. Create the Cargo workspace with `core` and `shell-macos`; toolchain pinned to 1.100; `objc2` crates pinned.
3. S1: the bare panel and hotkey; work through the table in 2.4 and the manual matrix.
4. S4 on the S1 binary as soon as it takes keys.
5. Start `tools/bench` and baseline your current launcher.
