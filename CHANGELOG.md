# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2025-02-09
- Add `Tab` completion to show possible completions and overwrite if singular [#134](https://github.com/wcampbell0x2a/heretek/pull/134)
- Show `-stack-list-frames` otherwise known as `Backtrace` when available [#129](https://github.com/wcampbell0x2a/heretek/pull/129)
- Add more documentation showing more usage of `heretek` in Hexdump and Normal usage [#128](https://github.com/wcampbell0x2a/heretek/pull/128)
- Try and deref the *entire* string when looking at a memory address [#127](https://github.com/wcampbell0x2a/heretek/pull/127)
- Update depends

## [0.4.0] - 2025-01-14
- Display registers that point to addresses in Hexdump [#115](https://github.com/wcampbell0x2a/heretek/pull/115)
- Show asm and function offset in asm deref [#117](https://github.com/wcampbell0x2a/heretek/pull/117)
- Expand `HERETEK_MAPPING_{START,END,LEN}` to allow optional index of mapping [#116](https://github.com/wcampbell0x2a/heretek/pull/116)
- Fix `HERETEK_MAPPING_{START,END,LEN}` to allow all ascii chars as filename [#116](https://github.com/wcampbell0x2a/heretek/pull/116)
- Add `--cmds` to cmd history [#118](https://github.com/wcampbell0x2a/heretek/pull/118)
- Ignore `#` comment lines in `--cmds` [#119](https://github.com/wcampbell0x2a/heretek/pull/119)

## [0.3.0] - 2025-01-09
- Adjusted size of UI elements in Main View [#102](https://github.com/wcampbell0x2a/heretek/pull/102)
- Add `--gdb-path` to override gdb executed [#101](https://github.com/wcampbell0x2a/heretek/pull/101)
- Show `Running` in status [#106](https://github.com/wcampbell0x2a/heretek/pull/106)
- Allow `control+c` to send `SIGINT` to process [#106](https://github.com/wcampbell0x2a/heretek/pull/106)
  - Always use `mi-async`
  - Override `continue` into `-exec-continue`
  - Override `stepi` into `-exec-step-instruction`
  - Override `step` into `-exec-step`
- Change `--cmd` into `--cmds` and from using `gdb> source` to just running each line as a gdb cmd [#106](https://github.com/wcampbell0x2a/heretek/pull/106)
- Add `--log-path` to control log location and remove writing to `app.log` by default [#108](https://github.com/wcampbell0x2a/heretek/pull/108)
- Change `RUST_LOG` env to `info` as default [#108](https://github.com/wcampbell0x2a/heretek/pull/108)

## [0.2.0] - 2025-01-02
- Remove `--local` argument, `heretek` now runs gdb locally by default [#96](https://github.com/wcampbell0x2a/heretek/pull/96)
- hexdump: Resolve `~/` in path for Saving [#97](https://github.com/wcampbell0x2a/heretek/pull/97)
- hexdump: Speed up by only computing what is needed for display [#98](https://github.com/wcampbell0x2a/heretek/pull/98)
- output/memory/hexdump: Add `G` hotkey to goto bottom [#98](https://github.com/wcampbell0x2a/heretek/pull/98)

## [0.1.0] - 2025-01-01
- Initial Release
