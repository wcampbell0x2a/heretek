# Update screenshot
```sh
$ cargo r --profile=dist -- --cmds test-sources/test.source
$ wmctrl -lx
$ import -window {id} images/screenshot.png
```

# Update vhs
```sh
$ vhs docs/vhs/main.tape
$ vhs docs/vhs/hexdump.tape
```
