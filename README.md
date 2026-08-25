# mvitop

`mvitop` is a fast, keyboard-driven terminal system monitor for Apple Silicon Macs. It reads CPU and unified-memory counters through Mach, system data through sysctl, real GPU utilization from the Apple GPU driver's IORegistry entry, and active GPU process time from a single privileged `powermetrics` stream.

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

- Clear CPU utilization, GPU utilization, and unified-memory gauges
- Total and per-core CPU utilization, including P/E core counts when macOS publishes them
- Apple-style unified memory details: used, available, wired, cached, compressed, and swap
- Real Apple GPU device, renderer, and tiler utilization when the driver publishes `PerformanceStatistics`
- A focused process table containing only processes active on the GPU in the latest one-second sample
- Per-process GPU time, with PID, command, filter, details, marking, and safe signal confirmation
- Fixed-size CPU utilization, GPU utilization, and unified-memory histories
- Responsive layouts for small terminals
- The TUI stays unprivileged; only the fixed `powermetrics` child command runs through `sudo`
- Terminal restoration on normal exit, error, panic, SIGINT, and SIGTERM

## Requirements

- An Apple Silicon Mac
- macOS
- Administrator authorization for the GPU process table

The prebuilt Homebrew release does not require a Rust toolchain. At launch, mvitop requests administrator authorization before entering the TUI because macOS restricts per-process GPU time. The mvitop UI, filtering, detail view, and signal actions continue to run as the invoking user.

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

Before the TUI opens, macOS may ask for your password through `sudo`. mvitop uses the resulting authorization only to run this fixed command:

```sh
/usr/bin/powermetrics --sample-rate 1000 --sample-count -1 \
  --buffer-size 1 --format plist --samplers tasks --show-process-gpu
```

If authorization is declined, CPU, GPU, unified-memory, and history metrics remain available; the GPU process table reports that authorization is required.

## Keyboard controls

| Key | Action |
|---|---|
| `q` | Quit |
| `?` / `h` | Help |
| `↑`, `↓`, `j` | Select a process |
| `PgUp` / `PgDn` | Move by a page |
| `g` / `p` | Sort by GPU time or PID |
| `/` | Edit process filter |
| `Enter` | Process detail |
| `Space` | Mark/unmark a process |
| `k` | Open the signal confirmation menu |
| `r` | Cycle UI refresh through 100/200/500/1000 ms |
| `Esc` | Close a dialog or finish filter editing |

The signal dialog supports SIGTERM, SIGINT, and SIGKILL. Before calling `kill(2)`, mvitop reads the target again and compares its start time to guard against PID reuse. PID 1 and mvitop itself are excluded.

## Metric accuracy and macOS limitations

`mvitop` shows missing data as `N/A`; it does not invent or extrapolate unavailable values.

- **GPU utilization:** read from the numeric device, renderer, and tiler utilization values in the Apple GPU driver's IORegistry `PerformanceStatistics` dictionary. These driver-published properties exist on the tested Apple Silicon generations but are not a stable cross-version API contract. Missing renderer or tiler values are simply omitted.
- **Per-process GPU activity:** read from `gputime_ms_per_s` in the plist output of macOS `powermetrics --show-process-gpu`. Rows with zero GPU time are omitted. This option is available only on supported hardware and requires administrator authorization.
- **GPU time vs. GPU utilization:** process GPU time is time scheduled in the one-second measurement window. It is not presented as device utilization, and individual rows are not expected to add up to the global GPU utilization value.
- **GPU memory:** not shown separately because Apple Silicon uses unified memory and macOS does not publish a reliable per-process GPU allocation through this interface.
- **Memory pressure:** shown as `N/A` because macOS does not expose a stable unprivileged numeric pressure percentage. Used memory follows macOS VM categories instead of pretending to be Linux `MemAvailable`.
- **CPU frequency:** not displayed because the available values are not consistently live or comparable across P/E clusters.
- **Protected processes:** macOS privacy and system protections can hide argv, executable paths, or working directories. The row remains visible with the fields that were obtainable.

## Architecture and sampling

The UI performs no metric I/O. Five named workers publish immutable, latest-only snapshots with `ArcSwap`; unchanged process vectors are shared with `Arc` instead of copied on every CPU update. An RCU update prevents simultaneous collectors from overwriting one another. The process worker keeps one `powermetrics` child alive and parses its NUL-separated plist samples instead of spawning a command every second.

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

## Automated releases

Changes land through normal pull requests using Conventional Commit prefixes such as `feat:`, `fix:`, and `docs:`. After a pull request is merged into `main`, [Release Please](https://github.com/googleapis/release-please-action) creates or updates a release pull request containing the next semantic version, `Cargo.toml`, `Cargo.lock`, and `CHANGELOG.md` changes.

Merging the release pull request creates the version tag and GitHub Release. A separate macOS workflow then verifies the tag against the Cargo package version, runs the full checks, builds and attests the Apple Silicon archive, attaches it to the release, and requests an immediate update from `kimata1007/homebrew-tap`.

Release automation uses two narrowly scoped Actions secrets instead of granting the repository-wide `GITHUB_TOKEN` permission to approve pull requests:

- `RELEASE_PLEASE_TOKEN`: a fine-grained token scoped only to `kimata1007/mvitop`, with Contents, Pull requests, and Issues read/write.
- `HOMEBREW_TAP_TOKEN`: a fine-grained token scoped only to `kimata1007/homebrew-tap`, with Contents read/write for repository dispatch.

If cross-repository dispatch is temporarily unavailable, the tap's scheduled updater checks the latest signed release every six hours and provides an automatic fallback.

## License

MIT
