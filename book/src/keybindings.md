# Keybindings

## Modes

heretek has two input modes:

- **Normal mode** — Navigation keys are active. This is the default.
- **Editing mode** — Keyboard input goes to the command line.

Press `i` to enter editing mode. Press `Esc` to return to normal mode.

## Global Keys

These work in any mode:

| Key | Action |
|-----|--------|
| `Ctrl+C` | Interrupt running program (sends `-exec-interrupt`) |
| `F1`–`F9` | Switch to corresponding tab |
| `Enter` | Send command (editing) or repeat last command (normal) |
| `Up` / `Down` | Navigate command history |
| `Tab` (editing) | GDB tab completion |
| `Tab` (normal) | Cycle to next tab |

## Normal Mode

| Key | Action |
|-----|--------|
| `i` | Enter editing mode |
| `q` | Open quit confirmation |

## Editing Mode

| Key | Action |
|-----|--------|
| `Esc` | Return to normal mode |
| `Tab` | GDB tab completion |
| `Enter` | Send command to GDB |

## Navigation Keys (Normal Mode)

These vim-style keys work across scrollable views:

| Key | Action |
|-----|--------|
| `j` | Scroll / move down 1 |
| `k` | Scroll / move up 1 |
| `J` | Scroll / move down 50 |
| `K` | Scroll / move up 50 |
| `g` | Jump to top |
| `G` | Jump to bottom |

> **Note**: `g` and `G` are available in Output, Mapping, Hexdump, Symbols, and Source views. Main and Register views only support `j/k/J/K`.

## View-Specific Keys

### Memory Mapping (F6)

| Key | Action |
|-----|--------|
| `H` | Open selected mapping in Hexdump |

### Hexdump (F7)

| Key | Action |
|-----|--------|
| `H` | Load heap into hexdump |
| `T` | Load stack into hexdump |
| `S` | Save hexdump bytes to file |

### Symbols (F8)

| Key | Action |
|-----|--------|
| `/` | Activate fuzzy search |
| `Enter` | Disassemble selected symbol |
| `Esc` | Close disassembly / cancel search |
| `r` / `R` | Refresh symbol list |

## Command History

- Up to 100 commands are stored in history
- `Up` / `Down` arrows navigate through previous commands
- Pressing `Enter` with an empty input repeats the last command

## Tab Completion

In editing mode, press `Tab` to trigger GDB tab completion:
- If one match: input is auto-completed
- If multiple matches: candidates are listed above the input box
