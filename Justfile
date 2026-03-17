list:
    just --list

run:
    #!/bin/bash -xe
    cargo xtask install
    RUST_LOG=${RUST_LOG:-info} ship serve
