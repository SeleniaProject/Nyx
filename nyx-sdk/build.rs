use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Disabled tonic_build to remove C dependencies
    // tonic_build::configure()
    //     .build_client(true)
    //     .build_server(false)
    //     .compile(&["../nyx-daemon/proto/control.proto"], &["../nyx-daemon/proto"])?;
    Ok(())
} 