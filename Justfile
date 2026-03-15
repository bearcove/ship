list:
    just --list

run:
    #!/bin/bash
    cargo xtask install
    RUST_LOG=${RUST_LOG:-info} ship serve
