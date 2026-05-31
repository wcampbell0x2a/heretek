# Usage

```console
GDB TUI Dashboard for the understanding of vast knowledge

Usage: heretek [OPTIONS]

Options:
      --gdb-path <GDB_PATH>
          Override gdb executable path

  -r, --remote <REMOTE>
          Connect to nc session

          `mkfifo gdb_pipe; cat gdb_pipe | gdb --interpreter=mi | nc -l -p 12345 > gdb_pipe`

      --ptr-size <PTR_SIZE>
          Switch into 32-bit mode

          Heretek will do it's best to figure this out on it's own, but this
          will force the pointers to be evaluated as 32 bit

          [default: auto]
          [possible values: 32, 64, auto]

  -c, --cmds <CMDS>
          Execute GDB commands line-by-line from file

          lines starting with # are ignored

      --log-path <LOG_PATH>
          Path to write log

          Set env `RUST_LOG` to change log level

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Local Session

Simply run heretek to start a local GDB session:

```
$ heretek
```

## Remote Session

See [Remote Targets](./remote.md) for connecting to remote GDB sessions.

## Command File

Use `-c` to execute GDB commands from a file on startup. Lines starting with `#` are ignored.

```
$ heretek -c my_commands.txt
```
