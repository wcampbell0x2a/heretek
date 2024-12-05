# heretek
Yet Another gdb TUI. Connect remotely with gdb when you don't have a working `gdbserver`, and show a tui inspired by `gef`.

* **No gdbserver requirements**: Many vendors ship invalid `gdbserver` binaries, this works on remote targets with just `gdb`, `nc`, and `mkfifo`.
* **No python requirements**: Many vendors ship `gdb` without python support

![screenshot](images/screenshot.png)
