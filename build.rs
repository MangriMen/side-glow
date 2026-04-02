#[cfg(windows)]
use winres::WindowsResource;

fn main() -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let mut res = WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile()?;
    }
    Ok(())
}
