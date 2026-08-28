# Acknowledgements

`mvitop` stands on the ideas and experience of terminal GPU-monitoring projects that came before it.

## nvtop

[nvtop](https://github.com/Syllo/nvtop), maintained by the nvtop contributors, established a highly effective visual language for terminal GPU monitoring: dense device summaries, utilization bars, historical graphs, process tables, and keyboard-first interaction.

nvtop is distributed under the GNU General Public License, version 3 or later. Its name, source code, and copyrights belong to its respective authors and contributors.

## nvitop

[nvitop](https://github.com/XuehaiPan/nvitop), created and maintained by Xuehai Pan and its contributors, further demonstrated the value of readable numerical summaries, adaptive compact layouts, high-density history views, flexible process sorting, and detailed process inspection.

nvitop's CLI and TUI are distributed under the GNU General Public License, version 3. Its public API components are distributed under the Apache License, version 2.0. Its name, source code, and copyrights belong to its respective authors and contributors.

## mvitop's relationship to these projects

mvitop is an Apple Silicon-focused implementation written in Rust. The current source tree does not vendor or link source code from nvtop or nvitop. It adapts the broader product lessons of those projects to macOS-native data sources and Apple Silicon concepts such as unified memory and a single integrated SoC.

If a future contribution adapts GPL-covered source code from either project, that contribution must identify its provenance and preserve all copyright and license notices required by GPLv3. This acknowledgement does not replace those obligations.

mvitop is not affiliated with, sponsored by, or endorsed by the nvtop or nvitop projects.
