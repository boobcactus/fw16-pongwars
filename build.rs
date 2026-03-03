fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set_manifest_file("app.manifest");
        res.compile().expect("Failed to compile Windows resources");
    }
}
