# Hexdump Demo
Output book/src/images/hexdump_view.gif
Set Theme "ayu"
Set Width 1920
Set Height 1080
Set FontSize 14
Set TypingSpeed 50ms

Hide
Type "./target/release/heretek"
Enter
Sleep 2s
Type "i"
Sleep 200ms
Type "file book/vhs/demo_dyn"
Enter
Sleep 500ms
Type "break 17"
Enter
Sleep 500ms
Type "run"
Enter
Sleep 3s
Escape
Sleep 500ms

# Switch to hexdump tab: Main -> Reg -> Stack -> Instr -> Output -> Mapping -> Hexdump
Tab
Sleep 100ms
Tab
Sleep 100ms
Tab
Sleep 100ms
Tab
Sleep 100ms
Tab
Sleep 100ms
Tab
Sleep 500ms

# Now use H to load heap
Type "H"
Sleep 2s
Show

Sleep 2s

# Scroll down
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms

Sleep 1s

# Jump to top
Type "g"
Sleep 1s

# Jump to bottom
Type "G"
Sleep 1s

# Load stack with T
Type "T"
Sleep 3s
