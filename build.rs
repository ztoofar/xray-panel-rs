fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);
    tonic_build::configure()
        .build_server(false)
        .compile_protos(&["proto/stats.proto", "proto/router.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/stats.proto");
    println!("cargo:rerun-if-changed=proto/router.proto");
    Ok(())
}
