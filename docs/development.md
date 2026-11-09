# Development log

This log records design decisions, implementation details, and the checks performed at the time of each change.

## 2026-11-06 — Stack choice and window prototype

Bruno requested Rust. The prototype used egui 0.37 through `eframe` to draw the overlay in one process without an embedded browser. The alternatives were Tauri 2.12.5 with React in WebView2, offering more visual flexibility at a memory cost, and Slint 1.18.1 with its own UI language. Versions were checked on crates.io that day.

- **Transparency.** `ViewportBuilder::with_transparent(true)` and a `clear_color` of `[0.0; 4]` leave only the rail visible.
- **Position and layering.** `with_always_on_top()`, `with_decorations(false)`, and `with_taskbar(false)` create the overlay. It computes the right-edge position on the first frame, once egui knows the monitor size.
- **Click-through.** `ViewportCommand::MousePassthrough(true)` lets clicks pass to the window underneath. It also prevents the overlay from receiving hover events, so the app polls `GetCursorPos` through `windows-sys` every 100 ms and disables click-through over the rail.
- **Details.** Hovering over the rail draws a rectangle and text to its left.

Windows 11 still drew a border, rounded corners, a shadow, and a gray line one pixel high. `DwmSetWindowAttribute` removed the shadow, rounded corners, and border color. Clearing `WS_CAPTION`, `WS_THICKFRAME`, and `WS_SYSMENU` removed the frame styles left by winit. The top line disappeared once the app reapplied these settings on each click-through change, when winit recalculated the styles.

On a 1920×1080 Windows 11 display, CPU use ranged from 0.05% to 0.15% with redraws every 100 ms. The executable was 5.4 MB and private memory was about 100 MB, apparently due to the OpenGL driver with the `glow` backend. Screenshots and `GWL_EXSTYLE` confirmed `WS_EX_TRANSPARENT` was on outside the rail and off over it. Display scaling other than 100% and multiple monitors were still untested.

Quota reading was still pending. Brunarias Dock already read Codex session logs from `~/.codex/sessions`. The Claude CLI had no valid login on this machine, and the desktop app had never supplied quota through the statusline.

## 2026-11-07 — Claude quota

The Claude desktop app supplied credentials to its child process through the environment, refreshed them itself, and produced neither a CLI credential file nor a statusline reading. Of six monitors reviewed through their READMEs (CodexBar, Usage Monitor for Claude, ClaudeMeter, claudeusagewin, claude-usage-tray, and Pulse), most used the terminal login and ran the CLI to refresh it. The desktop cookie appeared as a fallback. Bruno chose the terminal login.

- Claude Code CLI 2.1.319 was installed through its official Windows installer at `%USERPROFILE%\.local\bin\claude.exe`, outside PATH. Bruno ran `claude auth login` once.
- `src/claude.rs` reads `%USERPROFILE%\.claude\.credentials.json` and uses a token with more than a minute left to call the undocumented `https://api.anthropic.com/api/oauth/usage` endpoint with `anthropic-beta: oauth-2025-04-20`.
- On expiration or HTTP 401, the app runs `claude update` with `CREATE_NO_WINDOW` and tries again once. This refresh technique came from Usage Monitor for Claude. The child process drops `CLAUDE*` environment variables because a parent Claude desktop session can change how the CLI handles login.
- The parser handles both the new `limits` array (`session`, `weekly_all`, `weekly_scoped`) and the old `five_hour` and `seven_day` fields.
- A worker reads quota every three minutes and requests a redraw. The card shows usage and the local reset time using `chrono`. The original `aneis-de-cota --claude` command printed one reading without opening the overlay.

Six tests covered valid, expired, nearly expired, missing or malformed credentials and the two response shapes. Three failed against stubs as expected (RED). The three rejection tests already passed. All six passed after implementation (GREEN).

The live reading showed 26% for five hours and 21% for seven days. With the credential expiry manually set five minutes in the past, the CLI refreshed it and the app read quota in 3.6 seconds. The rings and card showed the live values and reset time. Private memory appeared to rise from roughly 100 MB to 180 MB after the network request. The cause was investigated next.

## 2026-11-07 — Memory

Task Manager's Memory column shows the private working set, the part resident in RAM. Earlier figures referred to private committed memory. Measurements at 20 seconds:

| Case | Task Manager | Private committed |
|---|---:|---:|
| `glow`, fixed ring values, no network | 53 MB | 101 MB |
| `glow`, fixed values delivered by a worker after one second | 53 MB | 102 MB |
| `glow`, live network reading | 85 MB | 102 MB |
| `wgpu`, fixed values | 116 MB | 384 MB |
| `wgpu`, live reading | 95 MB | 351 MB |

The `wgpu` backend loaded AMD's Vulkan, DirectX 12, and OpenGL drivers and was dropped. Most of the `glow` footprint came from AMD's OpenGL driver (`atio6axx.dll`). A minimal app making the same HTTPS call with `ureq` took under 1 MB. Trimming unused pages with `EmptyWorkingSet` took both `glow` cases down to 3 MB before they settled at 7–8 MB. Windows had retained pages touched during driver startup and the first connection.

The app began calling `K32EmptyWorkingSet` two seconds after each reading. Measurements were 2.4 MB at 3 seconds, 7.9 MB at 30 seconds, 8.8 MB at 60 seconds, and 5.7 MB after hover, with 0.01% CPU. The rings and card still rendered normally. Private committed memory remained near 100 MB, mostly from the video driver.

## 2026-11-09 — Codex quota

Codex writes a `token_count` event with `rate_limits` to `%USERPROFILE%\.codex\sessions\YYYY\MM\DD\rollout-*.jsonl`. Each window includes `used_percent`, `window_minutes` (300 for five hours, 10080 for seven days), and `resets_at` in Unix seconds. This is also Brunarias Dock's source and works without a network request or login. The app reads the Windows logs. Bruno had stopped using Codex through WSL in October.

- `src/quota.rs` defines the shared quota window. `src/codex.rs` checks up to eight recent sessions and takes the last non-null `rate_limits`, skipping invalid lines.
- A worker reads both sources every three minutes. A source can fail independently, with its error displayed in the card.
- Initially, a reset after the last recorded reading left the usage unknown. The ring showed `?` without an arc and the card asked the user to open Codex. The rule came from the Dock and also applied to Claude.
- The overlay grew to 340×320. The 268 px card, gap, and rail took up 332 px, which had clipped the text at the previous 320 px width. The dark rail height followed the number of rings.
- `--ler` replaced `--claude`, printed both sources, and marked stale values.

Of seven tests written first, four failed against the stubs for the expected reasons (RED). Three stubs already returned the expected empty or fresh result. All 13 tests passed after implementation (GREEN). Expected dates were calculated independently in Python. At home, the CLI showed Claude at 20% for five hours and 31% for seven days, and Codex at 9% for seven days with a stale five-hour window. A screenshot showed four rings, the third with `?`, and the full hover card.

The first comparison appeared to show 21 MB versus 10 MB before Codex. Running both versions side by side for four minutes showed similar cycles. Memory rose about 1 MB per 30 seconds to 12–17 MB, then fell to 4.5 MB at the next working-set trim. The earlier 7–8 MB estimate covered only the first minute. Usage outside this Windows installation is absent from its logs.

## 2026-11-09 — Visual design

Bruno approved an HTML mockup for idle, hover, and alert after asking for the original app icons. The rail gained an icon and block per agent, a separator, a percentage inside each ring, and a `5h` or `7d` label below it. The installed 48 px Claude tile and Codex icon were copied to `assets/` and embedded with `include_bytes!`. They belong to Anthropic and OpenAI. At that point the repository was local. The `image` crate was added with PNG support only.

The idle rail used 45% opacity and brightened over 0.25 seconds when the pointer was within 120 px or a fresh window exceeded 85%. The card showed an agent heading, a colored bar per window, the percentage, and relative reset time or weekday and time. Errors wrapped within the agent's section. The app loaded Segoe UI and Segoe UI Semibold from `C:\Windows\Fonts`, with egui fonts as a fallback.

Three reset-text tests covered under an hour, under a day with and without minutes, and a day or more. They failed against a stub and passed after implementation, bringing the total to 16. Python independently confirmed the weekday. Screenshots verified idle, pointer proximity, and hover. No live quota reached the alert threshold. Memory measured 6.2 MB at 10 seconds, 8.1 MB at 30 seconds, and 12 MB at 60 seconds. The executable grew to 8.1 MB.

## 2026-11-09 — Frame flicker

Bruno saw a one-frame title bar and white border as the pointer entered or left the rail. The winit implementation of egui's `MousePassthrough` reapplied all styles when toggling `WS_EX_TRANSPARENT | WS_EX_LAYERED`, briefly restoring the frame.

The app began toggling the two bits through `SetWindowLongPtrW(GWL_EXSTYLE)` and updating its internal state only after finding the window. Frame cleanup checks every frame and only acts when the frame returns. Winit can restore those styles in the first few frames after startup.

An `ffmpeg` `gdigrab` recording at 60 fps captured eight pointer transitions per version. The mean brightness of the top 32 px exceeded 100 with the title bar and normally stayed between 40 and 52.

| Version | Frames | Frames with title bar | Peak brightness |
|---|---:|---:|---:|
| Before (`4d319a2`) | 455 | 2 | 164 |
| After | 454 | 0 | 50 |

The window style was `96030000` after the fix. The extended bits were `C0038` outside the rail and `40018` over it. All 16 tests passed. The recording verified the Windows-specific fix.

## 2026-11-09 — Smaller rail and Antigravity

Bruno approved a mockup with a smaller rail, only five-hour windows at rest, and Antigravity as the third agent. Antigravity quota is served by its running `language_server.exe`. The approach followed CodexBar's `docs/antigravity.md`:

1. Find the `language_server*` process with `--app_data_dir antigravity` in its command line.
2. Extract `--csrf_token` and find the process's listening ports.
3. Send `POST https://127.0.0.1:<port>/exa.language_server_pb.LanguageServerService/RetrieveUserQuotaSummary` with `{}`, `X-Codeium-Csrf-Token`, and `Connect-Protocol-Version: 1`.

The response groups Gemini, Claude and GPT, and Gemini 4 Argon into five-hour and weekly windows with `remainingFraction` and `resetTime`. The Google login route was dropped after a Google forum thread reported a possible account suspension.

`src/antigravity.rs` obtains the process command line through `NtQueryInformationProcess` class 60 (`ProcessCommandLineInformation`) without reading the other process's memory or invoking PowerShell. It enumerates processes through `CreateToolhelp32Snapshot` and ports through `GetExtendedTcpTable`. TLS verification is disabled for that loopback call because the server uses its own certificate. Version 2.21.1 can change ports or briefly stop listening after launch, so the app tries all ports owned by that process. If Antigravity closes, the last good reading stays until it becomes stale. Starting with Antigravity closed prompts the user to open it. A missing protobuf JSON `remainingFraction` represents zero remaining.

The rail narrowed from 52 to 38 px and the rings from 38 to 28 px. Only five-hour windows appeared at rest, without labels. Proximity or high usage revealed the other windows. Antigravity used G5h, G7d, C5h, C7d, and A7d. The card widened to 290 px for the Argon label. The window position switched to `GetWindowRect` because egui briefly reported the default top-left position after launch, opening the rail if the pointer happened to be there.

Five new tests produced three expected failures against stubs (RED). Both rejection tests already passed. All 21 passed after implementation (GREEN). With Antigravity open, the CLI read five windows in 0.6 seconds, with Gemini and Claude and GPT at 0% and Argon at 9%. With it closed, the CLI displayed the prompt. Screenshots confirmed idle, proximity, and hover. Memory measured 7.3 MB at 10 seconds, 8.4 MB at 30 seconds, and 9.6 MB at 60 seconds. Retention after closing Antigravity was not observed on screen during that three-minute polling cycle.

## 2026-11-09 — Smooth expansion

Previously, only opacity animated. The rail now animates one value from 0 to 1 over 0.3 seconds with egui's `cubic_in_out`. Height, background, and opacity follow that value. Extra rings start with zero height and gradually push the rings below them down. They and their labels become visible in the second half of the motion and are clipped to the rail.

A test expecting the halfway height initially got 125 instead of 156 (RED). All 22 tests passed after the change (GREEN). A 60 fps recording showed about ten intermediate sizes over roughly 0.3 seconds.

## 2026-11-09 — Gemini-only Antigravity rail

At Bruno's request, `on_rail` filters Antigravity to the two Gemini windows before layout and drawing. The card still shows all five windows. At rest the rail shows Gemini's five-hour window, and when expanded it shows G5h and G7d. The new filter test failed first and all 23 tests passed after the change. The expanded rail and full card were checked on screen.

## 2026-11-09 — Capsules

Bruno chose capsules from three mockups using the day's data. Each agent got its own capsule, with 6 px gaps and an 18 px icon instead of 16 px. Rings gained a thicker tinted background and rounded ends drawn as two circles. The short labels switched from monospace to Segoe UI.

Each card row now has the name and percentage on top, a full-width bar below, and reset text at the bottom. A colored badge next to each agent shows the highest fresh usage. Stale windows show an em dash and a refresh prompt. The overlay grew from 530 to 640 px to hold ten rows. Rows grew from 44 to 48 px after the first screenshot showed the reset text too close to the next row.

The rail height test kept its halfway rule and used the new capsule dimensions. All 23 tests passed. Idle, proximity, and hover were checked with live data from all three agents. A .NET screenshot confirmed that an apparent mix-up in the initial captures came from their side-by-side seam.

## 2026-11-09 — Two-digit minutes

The reset text now shows a leading zero for minutes below ten after the hour, as in “in 3 h 04.” The corresponding `renew_text` test first failed with the unpadded value (RED). All 23 tests passed after the change. Exact hours and an hour-plus-minutes case were observed on screen. The leading zero itself was checked in the test because no live window had a single-digit minute at the time.

## 2026-11-09 — Rounded Antigravity icon

Bruno wanted the taskbar icon with its rounded white square and centered A. The previous crop removed the rounded corners and looked square at 18 px. The replacement was extracted from `Antigravity.exe` at 256 px with `PrivateExtractIcons` and reduced to 48 px without cropping. The result was checked on the rail.

## 2026-11-09 — Reset Codex windows count as unused

Bruno asked for a reset Codex five-hour window to show an empty ring and 0% instead of `?`. If Codex had run on this Windows machine since the reset, its session log would have recorded a new reading. Usage on other machines or in WSL remains outside this inference.

`codex::after_reset` turns a reset window into 0% with no next reset time. It runs each frame, and the card explains that there has been no use since reset. Antigravity keeps the stale-reading behavior. A new test verifies that an expired window changes while a fresh weekly window remains intact. It failed before the change and all 24 tests passed afterward. The rail and card were checked on screen.

## 2026-11-09 — Rail label spacing

Bruno found the 5h, 7d, and G5h labels cramped. The label had started only 0.75 px below the ring and ended beside the next one. It moved down 2.5 px and the expanded slot grew from 42 to 48 px, leaving roughly 3 px above and 5 px below. All 24 tests passed, including the height test using the slot constant. The spacing was checked on screen.

## 2026-11-09 — Review, new name, and repository

Bruno chose **Brunarias Rings of AI**. The package, executable, window title, local folder, and repository became `brunarias-rings-of-ai`.

The full project review with `ponytail-review` found seven issues, fixed before the first push:

1. Claude requests could hang quota reading for all three sources if the connection stalled mid-response. `ureq` 3.5.1 defaults to no timeout. The request now has a 15-second timeout, matching Antigravity.
2. The README still described Codex as pending. It gained the three sources, their limits, and `--ler`.
3. The Opus weekly window gained `O7d` to distinguish it from the general weekly window.
4. The 85% alert now counts only windows displayed on the rail.
5. Duplicate instances could find the same window by title. `CreateMutexW`, with `Win32_Security`, now limits the overlay to one instance.
6. Window, cursor, frame, click-through, and memory calls moved from `main.rs` to `src/win32.rs` without changing their behavior.
7. Codex session reads now examine the last 1 MB of files that can exceed 15 MB. If more than 1 MB follows the last quota event, the app can fall back to an earlier session.

The Opus label, alert, and tail-reading tests each failed for the expected reason before the fixes. All 27 tests passed afterward. Timeout, README, mutex, and code movement had no new automated tests. The CLI read all three agents in 0.6 seconds. Launching the executable twice left one process. With Claude at 89%, the rail stayed open and the card displayed correctly. With the rail continuously expanded, memory peaked at 38 MB and dropped to 14 MB every three minutes. The earlier idle peak was 15 MB. The effect of expansion alone was not isolated.

## 2026-11-09 — Centered ring numbers

An 8× screenshot showed digits 1.5–2 px below the ring center, while horizontal alignment was correct. Egui's `CENTER_CENTER` centers the entire line box. Segoe UI reserves space above digits for accents, shifting them down visually. `centered` moves the number, `?`, and `!` up by 0.17 times the font size. The next screenshot showed the digit centers aligned with the rings. All 27 tests passed.

## 2026-11-09 — English text and portable launch

Bruno requested English labels and documentation. Quota names, reset text, and errors are in English. The read-only CLI is documented as `--read`, with `--ler` retained for existing commands. The README separates launching a prebuilt executable from building with Rust and describes how to add a startup shortcut. Three updated reset-text tests first failed on the Portuguese strings, then passed with the full 27-test suite.

## 2026-11-09 — Unified rail and compact usage card

Bruno chose the visual reference from [Pulse PR #109](https://github.com/qunqin24/Pulse/pull/109). The Windows overlay uses a single dark rail, provider icons inside the rings, and usage percentages below them. Hovering over an agent opens a compact card with that agent's windows, usage bars, and reset times. Proximity still reveals additional windows and the existing alert keeps the rail expanded.

The hover-selection test failed before implementation and passed afterward with the full 28-test suite. A Windows screen capture confirmed the dark rail, live percentages, and the Claude usage card beside its rings.

## 2026-11-09 — One rail icon per provider

Each provider displays its icon only on its primary ring. Additional windows retain their percentages and labels. All 28 tests passed, and the expanded rail was checked on Windows with all three providers.

## 2026-11-09 — Restore icons on all rings

Bruno asked to restore the v0.2.0 rail. The overlay source matches that release again. All 28 tests passed and the Windows executable launched.

## 2026-11-09 — One quota window per provider

The slim black rail sits flush with the right edge and shows each provider's short window. A current weekly window above 90% takes its place. The hover card continues to show all windows. The selection test failed on the previous behavior and passed after the change. All 29 tests passed, and the rail and Claude hover card were checked on Windows.

The rail ends now curve into the screen edge, following the reference while keeping the rail on the right. Both ends were checked on screen in Windows. All 29 tests passed.
