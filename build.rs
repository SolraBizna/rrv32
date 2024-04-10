use std::{fs, io::Write};

fn update_crate_comment() {
    let readme = fs::read_to_string("README.md").unwrap();
    let mut librs = fs::read_to_string("src/lib.rs").unwrap();
    if librs.starts_with("/*!") {
        if let Some(index) = librs.find("\n*/\n\n") {
            librs.drain(..index + 5);
        }
    }
    let mut prefix = String::new();
    prefix += "/*!\n\n<!-- This doc comment is automatically regenerated \
    whenever README.md changes,\nand should not be edited manually. -->\n\n";
    prefix += &readme.replace(
        "(src/bin/ttybox.rs)",
        "(https://github.com/SolraBizna/rrv32/blob/main/src/bin/ttybox.rs)",
    );
    prefix += "*/\n\n";
    let librs = prefix + &librs;
    let mut f = fs::File::create("src/lib.rs^").unwrap();
    f.write_all(librs.as_bytes()).unwrap();
    drop(f);
    fs::rename("src/lib.rs", "src/lib.rs~").unwrap();
    fs::rename("src/lib.rs^", "src/lib.rs").unwrap();
    fs::remove_file("src/lib.rs~").unwrap();
}

fn main() {
    println!("cargo:rerun-if-changed=README.md");
    let readme_metadata = fs::metadata("README.md").unwrap();
    let librs_metadata = fs::metadata("src/lib.rs").unwrap();
    if librs_metadata.modified().unwrap() < readme_metadata.modified().unwrap()
    {
        update_crate_comment();
    }
}
