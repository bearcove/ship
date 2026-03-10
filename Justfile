list:
    just --list

run:
    RUST_LOG=debug,hyper_util=warn,reqwest=warn cargo run --bin ship -- serve
