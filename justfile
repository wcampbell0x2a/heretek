hello:
    printf '#include <stdio.h>\nvoid hello()\n{\n    printf("hello world\\n");\n}\nint main()\n{\n    hello();\n    return 0;\n}' > hello.c && gcc -g hello.c -o hello

update-screenshots: hello build
    vhs docs/vhs/main.tape
    vhs docs/vhs/hexdump.tape
    vhs docs/vhs/readme.tape

run: hello
    RUST_LOG=trace cargo r --release -- --cmds test-sources/readme.source --log-path heretek.log

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
