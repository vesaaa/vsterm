fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icons/windows/VsTerm.ico");
        res.set("ProductName", "VsTerm");
        res.set("FileDescription", "VsTerm");
        res.set("OriginalFilename", "VsTerm.exe");
        res.set("InternalName", "VsTerm");
        if let Err(err) = res.compile() {
            panic!("failed to embed Windows icon: {err}");
        }
    }
}
