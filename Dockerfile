FROM ekidd/rust-musl-builder:stable

ADD --chown=rust:rust . ./

CMD cargo build --release
