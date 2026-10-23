//! En Windows incrusta el icono y los datos de versión en el `.exe`.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/edenbord.ico");
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/edenbord.ico")
            .set("ProductName", "EdenBord")
            .set("FileDescription", "EdenBord, editor de bordados");
        res.compile()
            .expect("no se pudo incrustar el icono en el ejecutable");
    }
}
