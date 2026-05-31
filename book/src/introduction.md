# Introduction

**heretek** is a GDB TUI Dashboard inspired by `gef`, designed to seamlessly connect to remote targets even without a functioning `gdbserver`.

- **No python requirements**: Many vendors ship `gdb` without python support. heretek ships a single statically-linked musl binary.
- **Architecture agnostic**: heretek only uses information given by `gdb`, no extra code required!
- **No gdbserver requirements**: Many vendors ship invalid `gdbserver` binaries. heretek works on remote targets with just `gdb`, `nc`, `cat`, and `mkfifo`. No more wrestling with invalid or missing `gdbserver` binaries.

![screenshot](images/readme.gif)

> "To every problem, a solution lies in the application of tech-lore" - Ferrarch Asklepian, Warhammer 40,000: Mechanicus
