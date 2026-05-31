hello:
    printf '#include <stdio.h>\nvoid hello()\n{\n    printf("hello world\\n");\n}\nint main()\n{\n    hello();\n    return 0;\n}' > hello.c && gcc -g hello.c -o hello

demo:
    gcc -g -static book/vhs/demo.c -o book/vhs/demo
    gcc -g book/vhs/demo.c -o book/vhs/demo_dyn

update-screenshots: hello demo build
    vhs docs/vhs/main.tape
    vhs docs/vhs/hexdump.tape
    vhs docs/vhs/readme.tape
    vhs book/vhs/main_view.tape
    vhs book/vhs/registers_view.tape
    vhs book/vhs/hexdump_view.tape
    vhs book/vhs/symbols_view.tape
    vhs book/vhs/stack_view.tape
    vhs book/vhs/instructions_view.tape
    vhs book/vhs/source_view.tape
    vhs book/vhs/mapping_view.tape
    vhs book/vhs/tab_cycle.tape

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
