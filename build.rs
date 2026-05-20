fn main() {
  #[cfg(target_os = "windows")]
  {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/app-icons/icon.ico");
    res.compile().expect("failed to embed Windows icon resource");
  }
}
