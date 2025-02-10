update-screenshots:
    cargo build --release
    vhs docs/vhs/main.tape
    vhs docs/vhs/hexdump.tape
    vhs docs/vhs/readme.tape
    
