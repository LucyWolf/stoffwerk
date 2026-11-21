#!/bin/sh
# Baut Stoffwerk im Docker-Container rust:latest (auf dem Entwicklungsrechner gibt es keinen C-Linker).
# Aufruf: ./bauen.sh [cargo-argumente]   z.B. ./bauen.sh build --release
exec docker run --rm -u "$(id -u):$(id -g)" \
  -v "$HOME/.cargo/registry:/usr/local/cargo/registry" \
  -v "$PWD:/w" -w /w -e CARGO_TERM_COLOR=never \
  rust:latest cargo "${@:-build}"
