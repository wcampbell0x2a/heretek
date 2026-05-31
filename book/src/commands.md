# Commands and Variables

heretek intercepts some commands and translates them to GDB MI commands. All other input is passed directly to GDB.

## Intercepted Commands

These common GDB commands are intercepted and translated to their MI equivalents for proper async handling:

| You type | heretek sends | Notes |
|----------|---------------|-------|
| `r`, `run` | `-exec-run` | Also sets `mi-async on` and Intel syntax |
| `c`, `continue` | `-exec-continue` | |
| `si`, `stepi` | `-exec-step-instruction` | |
| `ni`, `nexti` | `-exec-next-instruction` | |
| `s`, `step` | `-exec-step` | |
| `n`, `next` | `-exec-next` | |
| `finish`, `fin` | `-exec-finish` | |
| `until <loc>` | passed through | Marks program as executing |
| `attach <pid>` | passed through | Also sets Intel syntax |
| `file <path>` | passed through | Extracts and saves filepath |
| `hexdump <addr> <len>` | `-data-read-memory-bytes` | Switches to Hexdump view |

All other commands (e.g., `break main`, `info registers`, `x/10x $rsp`) are sent directly to GDB.

## Arithmetic Expressions

Parenthesized expressions are evaluated before sending to GDB. This lets you do inline math:

```
hexdump $HERETEK_MAPPING_START_[heap] (0x1000 + 0x200)
```

The `(0x1000 + 0x200)` is evaluated to `0x1200` before the command is processed.

## Variables

For all commands, the following heretek variables are resolved and expanded before sending to GDB.

### `$HERETEK_MAPPING_START_{index}_{section}`

Resolve the start address of the mapping that fits `section`. The `index` is optional.

For example, `$HERETEK_MAPPING_START_[heap]` resolves the start of the heap mapping.

To pick a specific entry when multiple mappings match, use the index. For example, `$HERETEK_MAPPING_START_1_a.out` picks the second (1th) `a.out` mapping:

```
Start Address        End Address          Size                 Offset               Permissions          Path
0x00400000           0x00401000           0x00001000           0x00000000           r--p                 a.out
0x00401000           0x00479000           0x00078000           0x00001000           r-xp                 a.out
```

This would resolve to `0x00401000`.

### `$HERETEK_MAPPING_END_{index}_{section}`

Resolve the end address of the mapping that fits `section` at an optional `index`.

For example, `$HERETEK_MAPPING_END_[heap]`.

### `$HERETEK_MAPPING_LEN_{index}_{section}`

Resolve the length of the mapping that fits `section` at an optional `index`.

For example, `$HERETEK_MAPPING_LEN_[heap]`.

## Command History

- Up to 100 commands are stored in history
- Navigate with `Up` / `Down` arrow keys
- Press `Enter` on an empty input to repeat the last command
- Commands from `--cmds` file are also added to history

## Command File (`-c`)

Use the `-c` flag to execute commands from a file on startup:

```
$ heretek -c commands.txt
```

The file is read line by line. Lines starting with `#` are treated as comments and skipped. Each command is processed through the same pipeline as interactive input (variable expansion, expression evaluation, command interception).
