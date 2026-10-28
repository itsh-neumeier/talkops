use std::{
    env, fs,
    path::{Path, PathBuf},
};
fn collect(dir: &Path, root: &Path, files: &mut Vec<(String, PathBuf)>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("reference directory")
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, root, files)
        } else {
            files.push((
                path.strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/"),
                path.canonicalize().unwrap(),
            ));
        }
    }
}
fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("reference/fusionpbx/yealink");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    let mut source = String::from("static REFERENCE: &[(&str,&str)] = &[\n");
    for (name, path) in files {
        source.push_str(&format!(
            "({name:?}, include_str!({:?})),\n",
            path.to_str().unwrap()
        ));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("yealink_reference.rs"),
        source,
    )
    .unwrap();
    let crate_root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    for (directory, output, constant, bytes) in [
        (
            "reference/fusionpbx",
            "all_reference.rs",
            "ALL_REFERENCE",
            true,
        ),
        (
            "templates/fusionpbx",
            "native_files.rs",
            "NATIVE_FILES",
            false,
        ),
        (
            "templates/talkops",
            "authored_files.rs",
            "AUTHORED_FILES",
            false,
        ),
    ] {
        let root = crate_root.join(directory);
        println!("cargo:rerun-if-changed={}", root.display());
        let mut files = Vec::new();
        collect(&root, &root, &mut files);
        let mut source = format!(
            "static {constant}: &[(&str,{})] = &[\n",
            if bytes { "&[u8]" } else { "&str" }
        );
        for (name, path) in files {
            if directory == "reference/fusionpbx" && !name.contains('/') {
                continue;
            }
            source.push_str(&format!(
                "({name:?}, {}!({:?})),\n",
                if bytes {
                    "include_bytes"
                } else {
                    "include_str"
                },
                path.to_str().unwrap()
            ));
        }
        source.push_str("];\n");
        fs::write(
            PathBuf::from(env::var_os("OUT_DIR").unwrap()).join(output),
            source,
        )
        .unwrap();
    }
}
