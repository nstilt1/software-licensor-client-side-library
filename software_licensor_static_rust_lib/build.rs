#[cfg(feature = "build-protos")]
fn main() {
    prost_build::Config::new()
        .out_dir("src/generated")
        .compile_protos(
            &[
                "protos/software_licensor_local.proto",
            ], 
            &["protos/"])
        .unwrap();
}
#[cfg(not(feature = "build-protos"))]
fn main() {
    build_cxx();
}

fn build_cxx() {
    cxx_build::bridge("src/lib.rs")
        .std("c++17")
        .compile("software_licensor_client_side");
}