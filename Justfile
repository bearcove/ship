list:
    just --list

run:
    cargo xtask install
    RUST_LOG=debug,hyper_util=warn,reqwest=warn ship serve
