# Remote Targets

A key feature of heretek is the ability to connect to remote GDB sessions without requiring `gdbserver`. Many embedded and vendor-shipped environments have broken or missing `gdbserver` binaries — heretek works around this using only `gdb`, `nc`, `cat`, and `mkfifo`.

## Setting Up the Remote Side

On the remote target, set up a pipe and start GDB listening over netcat:

```
$ mkfifo gdb_pipe
$ cat gdb_pipe | gdb --interpreter=mi | nc -l -p 12345 > gdb_pipe
```

This creates a bidirectional pipe between GDB's MI interface and a network socket on port 12345.

## Connecting from heretek

On your local machine, connect to the remote session:

```
$ heretek --remote <host>:<port>
```

For example:

```
$ heretek --remote 192.168.1.100:12345
```

heretek will connect to the remote GDB session and display the TUI dashboard as if it were a local session.
