# Symbols Demo
Output book/src/images/symbols_view.gif
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
Type "break main"
Enter
Sleep 500ms
Type "run"
Enter
Sleep 3s
Escape
Sleep 1s

# Tab to Symbols (7 tabs)
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Sleep 500ms
# Refresh symbols
Type "r"
# Wait for info functions to complete
Sleep 5s
Show

Sleep 2s

# Scroll through symbols
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Sleep 1s

# Fuzzy search for helper
Type "/"
Sleep 500ms
Type@100ms "helper"
Sleep 1s
Enter
Sleep 1s

# Disassemble helper
Enter
Sleep 2s
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Type "j"
Sleep 200ms
Sleep 1s
Escape
Sleep 1s

# Search for main
Type "/"
Sleep 500ms
Type@100ms "main"
Sleep 1s
Enter
Sleep 1s
Enter
Sleep 3s
