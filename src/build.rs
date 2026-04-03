fn main() {
    println!("cargo:rerun-if-changed=TheBorker.manifest");
    let mut res = winres::WindowsResource::new();
    res.set_manifest_file("TheBorker.manifest");
    res.compile().unwrap();
}