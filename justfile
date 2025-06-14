update-screenshots:
    cargo build --release
    vhs docs/vhs/main.tape
    vhs docs/vhs/hexdump.tape
    vhs docs/vhs/readme.tape

run:
    RUST_LOG=trace cargo r --release -- --cmds test-sources/test.source --log-path heretek.log

# Matches .github
build:
    cargo build --release --bins
test: build
    cargo test --release
bench:
    cargo bench
lint:
    cargo fmt
    cargo clippy
