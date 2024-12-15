## Variables
For all commands into `gdb` or `heretek` itself, the following variables are resolved and expanded.
### `$HERETEK_MAPPING_START_{section}`
Resolve the first mapping that fits `section`. For example, `$HERETEK_MAPPING_START_[heap]`

### `$HERETEK_MAPPING_END_{section}`
Resolve the first mapping that fits `section`. For example, `$HERETEK_MAPPING_END_[heap]`

## Commands
### `hexdump`
After using `hexdump [addr] [len]`, the `Hexdump` page will contain the chosen bytes.

