// Compiles proto/earthnet.proto using protox (pure-Rust protobuf compiler) so the
// crate builds with no external `protoc` install required.
fn main() {
    let fds = protox::compile(["proto/earthnet.proto"], ["proto"])
        .expect("protox: failed to compile earthnet.proto");

    prost_build::Config::new()
        .compile_fds(fds)
        .expect("prost-build: failed to generate Rust types");

    println!("cargo:rerun-if-changed=proto/earthnet.proto");
}
