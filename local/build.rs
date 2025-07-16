use {std::io, winresource::WindowsResource};

fn main() -> io::Result<()> {
    WindowsResource::new()
        // This path can be absolute, or relative to your crate root.
        .set_icon("assets/p51-03.ico")
        .compile()?;
    Ok(())
}
