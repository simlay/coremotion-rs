use std::{
    env,
    path::PathBuf,
    fs,
    io::Error,
};

fn sdk_path(target: &str) -> Result<String, std::io::Error> {
    use std::process::Command;
    let sdk = if vec![
        "x86_64-apple-ios",
        "i386-apple-ios",
        "aarch64-apple-ios-sim",
    ]
    .contains(&target)
    {
        "iphonesimulator"
    } else if target == "aarch64-apple-ios"
        || target == "armv7-apple-ios"
        || target == "armv7s-apple-ios"
    {
        "iphoneos"
    } else if target == "aarch64-apple-darwin" {
        "macosx"
    } else {
        "macosx"
    };

    let output = Command::new("xcrun")
        .args(&["--sdk", sdk, "--show-sdk-path"])
        .output()?
        .stdout;
    let prefix_str = std::str::from_utf8(&output).expect("invalid output from `xcrun`");
    Ok(prefix_str.trim_end().to_string())
}

fn build(sdk_path: Option<&str>, target: &str) -> Result<(), Error> {
    // Generate one large set of bindings for all frameworks.
    //
    // We do this rather than generating a module per framework as some frameworks depend on other
    // frameworks and in turn share types. To ensure all types are compatible across each
    // framework, we feed all headers to bindgen at once.
    //
    // Only link to each framework and include their headers if their features are enabled and they
    // are available on the target os.
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");
    println!("cargo:rustc-link-lib=framework=CoreMotion");

    // See https://github.com/rust-lang/rust-bindgen/issues/1211
    // Technically according to the llvm mailing list, the argument to clang here should be
    // -arch arm64 but it looks cleaner to just change the target.
    let target = if target == "aarch64-apple-ios" {
        "arm64-apple-ios"
    } else {
        target
    };
    // Begin building the bindgen params.

    let target_arg = format!("--target={}", target);
    let mut clang_args = vec!["-x", "objective-c", "-fblocks", &target_arg];
    if let Some(sdk_path) = sdk_path {
        clang_args.extend(&["-isysroot", sdk_path]);
    }

    {
        let mut builder = bindgen::Builder::default();
        builder = builder
            .clang_args(&clang_args)
            .objc_extern_crate(true)
            .layout_tests(false)
            .allowlist_recursively(true)
            .allowlist_type(".CMMotionManager")
            .allowlist_type(".CM.*")
            .allowlist_type(".NSValue.*")
            .allowlist_type(".NSObject.*")
            // time.h as has a variable called timezone that conflicts with some of the objective-c
            // calls from NSCalendar.h in the Foundation framework. This removes that one variable.
            .blocklist_item("timezone")
            // https://github.com/rust-lang/rust-bindgen/issues/1705
            .blocklist_item("objc_object")
            .blocklist_type("NSUnit")
            .blocklist_type("FndrOpaqueInfo")
            .blocklist_type("HFSCatalogFolder")
            .blocklist_file("Foundation/NSUnit.h")
            .header_contents("CMMotionManager.h", "#include<CoreMotion/CMMotionManager.h>");

        // Generate the bindings.
        let bindings = builder.generate().expect("unable to generate bindings");

        // Get the cargo out directory.
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("env variable OUT_DIR not found"));

        eprintln!("OUT_DIR={}", out_dir.to_str().unwrap());
        let _ = fs::create_dir(out_dir.clone());

        // Write them to the crate root.
        bindings
            .write_to_file(out_dir.join("coremotion.rs"))
            .expect("could not write bindings");

        Ok(())
    }
}

fn main() -> Result<(), std::io::Error> {
    let target = std::env::var("TARGET").unwrap();

    let directory = sdk_path(&target).ok();
    build(directory.as_ref().map(String::as_ref), &target)?;

    Ok(())
}
