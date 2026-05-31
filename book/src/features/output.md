# Output (F5)

The Output view shows all GDB output and commands in a scrollable log.

## Display

- **Mini strip**: In all non-Output views, the last 10 lines of output are shown in a strip at the bottom, always auto-scrolled to the latest output
- **Full view (F5)**: The entire output history is shown in a scrollable, full-screen view

## Content

The output panel shows:
- Commands you type, echoed back
- GDB stdout responses
- heretek messages, prefixed with `h>` (e.g., `h> hexdump successfully written to /tmp/dump`)

Some GDB output is filtered and not shown:
- Memory map parsing output
- Endianness detection messages
- Language detection messages
- Symbol list accumulation output

## Keybindings

| Key | Action |
|-----|--------|
| `g` | Jump to top |
| `G` | Jump to bottom |
| `j` | Scroll down 1 line |
| `k` | Scroll up 1 line |
| `J` | Scroll down 50 lines |
| `K` | Scroll up 50 lines |
