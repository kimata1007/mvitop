# mvitop

`mvitop` is a fast, keyboard-driven terminal system monitor for Apple Silicon Macs. It reads CPU and unified-memory counters through Mach, processes through libproc, system data through sysctl, and real GPU utilization from the Apple GPU driver's IORegistry entry. It never launches `powermetrics` or another command in its sampling loop.

![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange)
![Platform](https://img.shields.io/badge/platform-Apple%20Silicon%20macOS-lightgrey)
![License](https://img.shields.io/badge/license-MIT-blue)

<p align="center">
  <img src="docs/mvitop-demo.gif" alt="mvitop terminal interface demo" width="100%">
</p>
<p align="center"><sub>Recorded from <code>mvitop --demo</code> with synthetic metrics and processes. No host or personal data is included.</sub></p>

## Inspiration and independent implementation

`mvitop` takes product and UX inspiration from [nvtop](https://github.com/Syllo/nvtop) and [nvitop](https://github.com/XuehaiPan/nvitop): in particular, their dense at-a-glance terminal layouts, familiar top-style process tables, and keyboard-first workflows.

`mvitop` is not a port, fork, translation, or derivative implementation of either codebase. No nvtop or nvitop source code was copied, adapted, linked, or included as a dependency. The implementation was designed and written from scratch in Rust using Ratatui, Crossterm, and macOS-native Mach, libproc, sysctl, and IOKit interfaces.

The MIT license in this repository applies to mvitop's independently authored code; it is not a relicensing of either project. nvtop is distributed under GPL-3.0-or-later, while nvitop's CLI/TUI is GPL-3.0 and its API components are Apache-2.0. Their copyrights, names, and licenses remain with their respective projects.

## Features

- Starts with an immediate empty frame; collectors never block the first draw
- Total and per-core CPU utilization, including P/E core counts when macOS publishes them
- Apple-style unified memory details: used, available, wired, cached, compressed, and swap
- Real Apple GPU device utilization when the driver publishes `PerformanceStatistics`
- PID, PPID, user, CPU, physical footprint, threads, state, runtime, command line, executable, and cwd
- Fixed-size CPU, GPU, and memory histories
- Process sort, filter, tree view, marking, details, and safe signal confirmation
- Responsive layouts for small terminals
- Graceful degradation per metric; no `sudo` required
- Terminal restoration on normal exit, error, panic, SIGINT, and SIGTERM

## Requirements

- An Apple Silicon Mac
- macOS

The prebuilt Homebrew release does not require a Rust toolchain or administrator privileges from mvitop itself. Homebrew's own macOS setup requirements still apply.

## Install

Homebrew is the recommended installation method:

```sh
brew install kimata1007/tap/mvitop
```

Homebrew will keep the tap and package up to date through the usual commands:

```sh
brew update
brew upgrade mvitop
```

Versioned Apple Silicon archives and SHA-256 checksums are also available from [GitHub Releases](https://github.com/kimata1007/mvitop/releases). Release archives include GitHub artifact attestations that can be verified with:

```sh
gh attestation verify mvitop-v*-aarch64-apple-darwin.tar.gz \
  --repo kimata1007/mvitop
```

## Build from source

Building requires macOS with the Xcode Command Line Tools and Rust 1.85 or newer.

```sh
git clone https://github.com/kimata1007/mvitop.git
cd mvitop
cargo build --release
install -m 755 target/release/mvitop /usr/local/bin/mvitop
```

Run it in an interactive terminal:

```sh
mvitop
```

## Keyboard controls

| Key | Action |
|---|---|
| `q` | Quit |
| `?` / `h` | Help |
| `↑`, `↓`, `j` | Select a process |
| `PgUp` / `PgDn` | Move by a page |
| `c` / `m` / `p` | Sort by CPU, memory, or PID |
| `g` | Request GPU sort; explains when per-process GPU data is unavailable |
| `/` | Edit process filter |
| `t` | Toggle process tree |
| `Enter` | Process detail |
| `Space` | Mark/unmark a process |
| `k` | Open the signal confirmation menu |
| `r` | Cycle UI refresh through 100/200/500/1000 ms |
| `Esc` | Close a dialog or finish filter editing |

The signal dialog supports SIGTERM, SIGINT, and SIGKILL. Before calling `kill(2)`, mvitop reads the target again and compares its start time to guard against PID reuse. PID 1 and mvitop itself are excluded.

## Metric accuracy and macOS limitations

`mvitop` shows missing data as `N/A`; it does not invent or extrapolate unavailable values.

- **GPU utilization:** read from the numeric `Device Utilization %` value in the Apple GPU driver's IORegistry `PerformanceStatistics` dictionary. This driver-published property exists on the tested Apple Silicon generations but is not a stable cross-version API contract. If Apple removes or renames it, only GPU sampling degrades.
- **GPU frequency, power, and temperature:** shown as `N/A` unless a future backend can obtain a reliable real value without root. mvitop does not derive these from utilization or repeatedly spawn `powermetrics`.
- **Per-process GPU and GPU memory:** shown as `--`. macOS does not expose a stable, accurate unprivileged API across OS and SoC generations, so processes are never assigned estimated GPU values.
- **Memory pressure:** shown as `N/A` because macOS does not expose a stable unprivileged numeric pressure percentage. Used memory follows macOS VM categories instead of pretending to be Linux `MemAvailable`.
- **CPU frequency:** not displayed because the available values are not consistently live or comparable across P/E clusters.
- **Protected processes:** macOS privacy and system protections can hide argv, executable paths, or working directories. The row remains visible with the fields that were obtainable.

## Architecture and sampling

The UI performs no metric I/O. Five named workers publish immutable, latest-only snapshots with `ArcSwap`; unchanged process vectors are shared with `Arc` instead of copied on every CPU update. An RCU update prevents simultaneous collectors from overwriting one another.

| Collector | Default period |
|---|---:|
| GPU | 350 ms |
| CPU | 500 ms |
| Memory | 500 ms |
| Processes | 1 s |
| Static/slow system data | 15 s |

All histories are bounded ring buffers. Shutdown uses a condition variable so even the 15-second worker exits immediately.

## Performance check

Run the built-in, non-interactive startup check:

```sh
cargo run --release -- --startup-benchmark
```

It reports the first Ratatui frame render, collector-runtime construction, and first CPU/memory snapshot times. Numbers depend on the terminal, build profile, and machine.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

The native FFI is intentionally isolated under `src/platform/macos/`. See the safety comments at every unsafe boundary.

## License

MIT
