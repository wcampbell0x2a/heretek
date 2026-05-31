# Main View Demo - stepping through a program
Output book/src/images/main_view.gif
Set Theme "ayu"
Set Width 1920
Set Height 1080
Set FontSize 14
Set TypingSpeed 50ms

Hide
Type "./target/release/heretek"
Enter
Sleep 2s
# Load binary
Type "i"
Sleep 200ms
Type "file book/vhs/demo"
Enter
Sleep 1s
# Set breakpoint
Type "break main"
Enter
Sleep 500ms
# Run
Type "run"
Enter
Sleep 2s
Escape
Show

Sleep 2s

# Step through with si
Hide
Type "i"
Show
Sleep 200ms
Type "si"
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
Type "si"
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
Type "si"
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
Type "ni"
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
Type "ni"
Enter
Sleep 1s
Hide
Escape
Show
Sleep 2s
