## Variables
For all commands into `gdb` or `heretek` itself, the following variables are resolved and expanded.

### `$HERETEK_MAPPING_START_{index}_{section}`
Resolve the mapping that fits `section`. For example, `$HERETEK_MAPPING_START_[heap]` since the `index` is optional.

For example, to pick the 1th(not the 0th) entry. Use for example, `$HERETEK_MAPPING_START_1_a.out`.
```
Start Address        End Address          Size                 Offset               Permissions          Path                                                                                                                                 █
0x00400000           0x00401000           0x00001000           0x00000000           r--p                 a.out                                                         █
0x00401000           0x00479000           0x00078000           0x00001000           r-xp                 a.out
```
That would resolve into `0x401000`.

### `$HERETEK_MAPPING_END_{index}_{section}`
Resolve the mapping that fits `section` at an optional `index`. For example, `$HERETEK_MAPPING_END_[heap]`.

### `$HERETEK_MAPPING_LEN_{index}_{section}`
Resolve the mapping that fits `section` at an optional `index`. For example, `$HERETEK_MAPPING_LEN_[heap]`.

## Commands
### `hexdump`
After using `hexdump [addr] [len]`, the `Hexdump` page will contain the chosen bytes.

