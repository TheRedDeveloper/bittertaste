fn main() {
  println!("cargo:rerun-if-changed=assets/images/logo.ico");
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/images/logo.ico");
    res.compile().expect("Failed to embed Windows icon resource");
  }
}
