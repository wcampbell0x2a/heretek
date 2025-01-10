# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2025-01-09
- Adjusted size of UI elements in Main View [#102](https://github.com/wcampbell0x2a/heretek/pull/102)
- Add `--gdb-path` to override gdb executated [#101](https://github.com/wcampbell0x2a/heretek/pull/101)
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
