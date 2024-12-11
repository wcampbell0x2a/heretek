# heretek
Yet Another gdb TUI. Connect remotely with gdb when you don't have a working `gdbserver`, and show a tui inspired by `gef`.

* **No gdbserver requirements**: Many vendors ship invalid `gdbserver` binaries, this works on remote targets with just `gdb`, `nc`, and `mkfifo`.
* **No python requirements**: Many vendors ship `gdb` without python support

```
Yet Another GDB TUI

Usage: heretek [OPTIONS]

Options:
  -l, --local            Run gdb as child process from PATH
  -r, --remote <REMOTE>  Connect to nc session
      --32               Switch into 32-bit mode
  -h, --help             Print help (see more with '--help')
  -V, --version          Print version 

```

![screenshot](images/screenshot.png)
