# Tooling
## Rust
This project uses the rust compiler. Follow instructions from [Installing Rust](rust-lang.org/tools/install).

## Justfile
This project includes a [justfile](justfile) for ease of development. [Installing Just](github.com/casey/just?tab=readme-ov-file#installation).
Hopefully this will eliminate errors before running the CI once your patch/merge request submitted!

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


See the [justflie](justfile) for more recipes!
