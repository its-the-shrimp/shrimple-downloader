#[allow(clippy::expect_used)]
fn main() {
    println!("cargo:rerun-if-changed=website");
    println!("cargo:rerun-if-changed=.cargo");
    assert!(
        std::process::Command::new("shrimple")
            .arg("website/index.html")
            .stdout(std::process::Stdio::null())
            .status().expect("`shrimple` should be in `$PATH`")
            .success(),
        "`shrimple website/index.html` exited unsuccessfully"  
    );
}
