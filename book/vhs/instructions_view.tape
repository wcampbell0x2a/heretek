# Instructions View Demo
Output book/src/images/instructions_view.gif
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
Sleep 500ms

# Tab to Instructions (3 tabs)
Tab
Sleep 500ms
Tab
Sleep 500ms
Tab
Show

Sleep 2s

# Step through instructions to show $pc moving
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
