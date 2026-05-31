# Memory Mapping Demo
Output book/src/images/mapping_view.gif
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
Type "file book/vhs/demo"
Enter
Sleep 500ms
Type "break 17"
Enter
Sleep 500ms
Type "run"
Enter
Sleep 3s
Escape
Sleep 1s

# Tab to Mapping (5 tabs)
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 2s
Show

Sleep 2s

# Navigate through mappings
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Sleep 1s

# Jump to bottom
Type "G"
Sleep 1s

# Jump to top
Type "g"
Sleep 1s

# Select a mapping and open in hexdump
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "H"
Sleep 3s
