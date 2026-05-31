# Development

All patches and merge requests are welcome!

## Tooling

### Rust

This project uses the Rust compiler. Follow instructions from [Installing Rust](https://www.rust-lang.org/tools/install).

### Justfile

This project includes a [justfile](https://github.com/wcampbell0x2a/heretek/blob/master/justfile) for ease of development. See [Installing Just](https://github.com/casey/just?tab=readme-ov-file#installation).

## Building

```console
$ just build
```

## Testing

Testing requires `gdb`. Install from your package manager.

```console
$ just test
```

## Linting

```console
$ just lint
```

See the [justfile](https://github.com/wcampbell0x2a/heretek/blob/master/justfile) for more recipes.
