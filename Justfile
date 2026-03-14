list:
    just --list

run:
    cargo xtask install
    RUST_LOG=${RUST_LOG:info} ship serve
