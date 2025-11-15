Output images/readme.gif
Set Theme ayu
Set Padding 2
Set FontSize 12
Set Width 1000
Set Height 1000

Type "./target/release/heretek"
Enter
Sleep 2s

Type "i"
Type "file test-assets/test_render_app/a.out"
Enter
Sleep 2s

Type "break main"
Enter
Sleep 2s

Type "run"
Enter
Sleep 5s

Type "si"
Enter
Sleep 5s
Type "si"
Enter
Sleep 5s
Type "si"
Enter
Sleep 5s

Escape
Tab
# registers
Sleep 2s
Tab
# stack
Sleep 2s
Tab
# instructions
Sleep 2s
Tab
# output
Sleep 2s
Tab
# mapping
Sleep 2s
Type "j"
Sleep 1s
Type "j"
Sleep 1s
Tab
# hexdump
Sleep 2s
Type "T"
Sleep 1s
Type "G"
Sleep 1s
Type "K"
Sleep 1s
Type "k"
Sleep 1s
Type "k"
Sleep 1s
Type "k"
Sleep 2s
Tab
# symbols
Type "r"
Sleep 2s
Type "j"
Sleep 2s
Type "j"
Sleep 2s
Type "j"
Sleep 2s
Enter
Type "d"
Sleep 2s
