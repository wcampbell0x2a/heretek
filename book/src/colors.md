# Color Coding

heretek uses the **Ayu** color theme throughout its interface. Colors have semantic meaning — they tell you what type of memory a value points to at a glance.

## Memory Region Colors

| Color | Hex | Meaning |
|-------|-----|---------|
| Green | `#aad94c` | Heap memory |
| Purple | `#d2a6ff` | Stack memory |
| Red | `#ff3333` | Code / text segment |
| Yellow | `#e6b450` | ASCII strings |
| Orange | `#ff8f40` | Assembly instructions |

These colors appear in:
- Register values and dereference chains
- Stack entry values
- The color legend in the title bar

## UI Element Colors

| Color | Used for |
|-------|----------|
| Blue (`#59c2ff`) | Table headers, input field (editing mode), output panel title |
| Purple (`#d2a6ff`) | Register names, stack addresses, instruction addresses |
| Orange (`#ff8f40`) | Panel titles, register annotations on stack, selected rows |
| Green (`#aad94c`) | Current instruction highlight, current source line, active tab |
| Red (`#ff3333`) | Changed registers (values that changed since last stop) |
| Yellow (`#e6b450`) | Popup titles (quit confirmation, hexdump save) |

## Hexdump Byte Colors

The hexdump view uses a different color scheme for individual bytes:

| Color | Byte type |
|-------|-----------|
| Dark gray | Null bytes (`0x00`) |
| Blue | Printable ASCII (graphic characters) |
| Green | ASCII whitespace (space, tab, newline) |
| Orange | ASCII control characters |
| Yellow | Non-ASCII bytes (`>= 0x80`) |

## Disabling Colors

Set the `NO_COLOR` environment variable to disable all TUI colors:

```
$ NO_COLOR=1 heretek
```
