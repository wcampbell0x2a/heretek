# heretek
[<img alt="github" src="https://img.shields.io/badge/github-wcampbell0x2a/heretek-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/wcampbell0x2a/heretek)
[<img alt="crates.io" src="https://img.shields.io/crates/v/heretek.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/heretek)
[<img alt="build status" src="https://img.shields.io/github/actions/workflow/status/wcampbell0x2a/heretek/main.yml?branch=master&style=for-the-badge" height="20">](https://github.com/wcampbell0x2a/heretek/actions?query=branch%3Amaster)

Yet Another gdb TUI. Connect remotely with gdb when you don't have a working `gdbserver`, and show a tui inspired by `gef`.

* **No gdbserver requirements**: Many vendors ship invalid `gdbserver` binaries, this works on remote targets with just `gdb`, `nc`, and `mkfifo`.
* **No python requirements**: Many vendors ship `gdb` without python support.
* **Architecture agnostic**: `heretek` only uses information given by `gdb`, no extra code required!

![screenshot](images/screenshot.png)

## Build
Either build from published source in crates.io.
```
$ cargo install heretek --locked
```

Or download from [github releases](https://github.com/wcampbell0x2a/heretek/releases).

## Usage
```console
Yet Another GDB TUI

Usage: heretek [OPTIONS]

Options:
  -l, --local            Run gdb as child process from PATH
  -r, --remote <REMOTE>  Connect to nc session
      --32               Switch into 32-bit mode
  -h, --help             Print help (see more with '--help')
  -V, --version          Print version 
```

## Info
See [commands](./docs/commands.md) for internal `heretek` commands.

> "To every problem, a solution lies in the application of tech-lore" - Ferrarch Asklepian, Warhammer 40,000: Mechanicus
