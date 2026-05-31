# Backtrace

The backtrace panel is not a standalone tab — it appears automatically between the main content area and the output strip whenever backtrace data is available.

## Display

Each frame in the call stack is shown as:

```
  0x00401238 → main
  0x7ffff7a2d840 → __libc_start_main
  0x00401060 → _start
```

- **Addresses** are shown in purple
- **Function names** are shown in orange
- Unknown functions display as `??`

## Behavior

- The backtrace is populated from `-stack-list-frames`, which is queried automatically every time the program stops
- The panel height adjusts to fit the number of frames
- When the program is running (no stop event), the backtrace panel disappears
