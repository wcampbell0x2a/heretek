# Update screenshot
```
$ cargo r --profile=dist -- --local --cmd "source test-sources/test.source"
$ wmctrl -lx
$ import -window {id} images/screenshot.png
```
