update-screenshots:
    cargo build --release
    vhs docs/vhs/main.tape
    vhs docs/vhs/hexdump.tape
    vhs docs/vhs/readme.tape

run:
    RUST_LOG=trace cargo r --release -- --cmds test-sources/test.source --log-path nice.log
