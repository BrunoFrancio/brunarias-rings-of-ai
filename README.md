# Brunarias Rings of AI

A Windows desktop overlay that shows how much of your Claude Code, Codex, and Antigravity quotas you have used. It is written in Rust and inspired by [Pulse](https://github.com/qunqin24/Pulse) for macOS. The project was originally called Anéis de cota.

## Run on Windows

Download the Windows x64 executable from [GitHub Releases](https://github.com/BrunoFrancio/brunarias-rings-of-ai/releases/latest). Put `brunarias-rings-of-ai.exe` in a folder you want to keep and double-click it. The app opens a rail along the right edge of the screen and allows one instance at a time. To start it at sign-in, press **Win+R**, open `shell:startup`, and add a shortcut to the executable.

To build it yourself, install Rust with the Windows MSVC toolchain and Visual Studio Build Tools with the **Desktop development with C++** workload. From the repository root, run:

```powershell
cargo build --release --locked
.\target\release\brunarias-rings-of-ai.exe
```

For development, use `cargo run --locked` and `cargo test --locked`. To read the quotas once in PowerShell without opening the overlay, run:

```powershell
.\target\release\brunarias-rings-of-ai.exe --read
```

## What the overlay shows

- At rest, a dim rail shows each agent's 5-hour window, with its icon inside the ring and usage underneath.
- Move the pointer near the rail to reveal additional windows and their short labels.
- Hover over an agent to see its windows, usage bars, and reset times in a compact card.
- A window on the rail at 85% usage or higher keeps the rail expanded.

Clicks outside the rail pass through to the window underneath.

## Quota sources

| Agent | Source | What to expect |
|---|---|---|
| Claude Code | Terminal login in `%USERPROFILE%\.claude\.credentials.json` and the endpoint used by `/usage` | Run `claude auth login` to sign in. The app calls `claude update` when the credentials expire. |
| Codex | Recent session logs in `%USERPROFILE%\.codex\sessions` | Run Codex on this Windows machine to refresh the data. A window that has reset since the last recorded use shows 0%. |
| Antigravity | The local server started by Antigravity | Keep Antigravity open to fetch fresh data. The overlay retains the last successful reading and marks expired windows as stale. |

These quota sources rely on undocumented provider interfaces and may change. The icons in `assets/` belong to the respective providers.

See the [development log](docs/development.md) for design decisions and verification results.
