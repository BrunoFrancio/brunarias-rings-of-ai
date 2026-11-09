# Brunarias Rings of AI

See your Claude Code, Codex, and Antigravity quotas at a glance in a slim overlay on the right edge of Windows. Each provider gets one ring. Hover over it for usage and reset times.

[Download](#download) · [Quick start](#quick-start) · [Using the app](#using-the-app) · [Supported tools](#supported-tools) · [Development](#development)

## Download

**[Download for Windows x64](https://github.com/BrunoFrancio/brunarias-rings-of-ai/releases/latest/download/brunarias-rings-of-ai.exe)** · [Release notes](https://github.com/BrunoFrancio/brunarias-rings-of-ai/releases/latest)

The download is a portable `.exe`. Save it in a folder you want to keep and double-click to launch.

## Quick start

1. Open `brunarias-rings-of-ai.exe`.
2. Use your AI tools as usual. See [supported tools](#supported-tools) for what each provider needs to report usage.
3. Hover over a ring to see that provider's usage windows and reset times.

To launch at sign-in, press **Win+R**, open `shell:startup`, and add a shortcut to the executable.

## Using the app

- Each ring normally shows the provider's short usage window. A current weekly window takes its place when weekly usage exceeds **90%**.
- At **85%** or higher, a visible ring keeps the rail bright.
- The hover card shows all available windows for the selected provider.
- Clicks outside the rail pass through to the window underneath. The overlay allows one instance at a time.

To print a one-time reading in PowerShell, run this from the executable's folder:

```powershell
.\brunarias-rings-of-ai.exe --read
```

## Supported tools

| Provider | What to do | Where usage comes from |
|---|---|---|
| Claude Code | Sign in with `claude auth login`. | The app uses the CLI login and the endpoint behind `/usage`. It calls `claude update` to refresh expired credentials. |
| Codex | Run Codex on this Windows machine to update its session logs. | The app reads recent local sessions. A window that reset after the last recorded use shows 0%. |
| Antigravity | Keep Antigravity open to fetch fresh readings. | The app reads Antigravity's local server and retains the last successful reading when it closes. |

Quota collection relies on undocumented provider interfaces that may change. The icons in `assets/` belong to the respective providers.

## Development

Install Rust with the Windows MSVC toolchain and Visual Studio Build Tools with the **Desktop development with C++** workload. From the repository root:

```powershell
cargo test --locked
cargo run --locked
cargo build --release --locked
```

The optimized executable is at `target\release\brunarias-rings-of-ai.exe`. The overlay is built with Rust and egui and was inspired by [Pulse](https://github.com/qunqin24/Pulse) for macOS. See the [development log](docs/development.md) for design decisions and verification.
