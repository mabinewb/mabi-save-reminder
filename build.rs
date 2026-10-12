fn main() {
    println!("cargo:rerun-if-changed=resources.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");
    embed_resource::compile("resources.rc", embed_resource::NONE)
        .manifest_required()
        .expect("Windows 아이콘 리소스 빌드 실패");
}
