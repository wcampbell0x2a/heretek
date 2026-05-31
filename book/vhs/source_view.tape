# Source View Demo
Output book/src/images/source_view.gif
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
Type "break main"
Enter
Sleep 500ms
Type "run"
Enter
Sleep 3s
Escape
Sleep 1s

# Tab to Source (8 tabs)
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
Tab
Sleep 2s
Show

Sleep 2s

# Scroll through source
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

# Step and see source line update
Hide
Type "i"
Show
Sleep 200ms
Type "n"
Enter
Sleep 1s
Hide
Escape
Show
Sleep 1s

Hide
Type "i"
Show
Sleep 200ms
Type "n"
Enter
Sleep 1s
Hide
Escape
Show
Sleep 1s

Hide
Type "i"
Show
Sleep 200ms
Type "n"
Enter
Sleep 1s
Hide
Escape
Show
Sleep 2s
