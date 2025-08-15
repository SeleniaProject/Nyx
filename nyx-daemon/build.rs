use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Disabled tonic_build to remove C dependencies
    // tonic_build::configure()
    //     .build_server(true)
    //     .compile(&["proto/control.proto"], &["proto"])?;
    Ok(())
}
