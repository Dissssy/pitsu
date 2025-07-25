use std::io;

fn main() -> io::Result<()> {
    if std::env::var("CARGO_CFG_TARGET_OS").map_err(io::Error::other)? == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/p51-03.ico");
        res.compile()?;
    }
    Ok(())
}
