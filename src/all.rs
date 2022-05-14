use cranelift_reader::{parse_test, ParseOptions};
use std::fs::{self, DirEntry};
use std::io;

use std::path::Path;

// one possible implementation of walking a directory only visiting files
fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }
    Ok(())
}

fn main() {
    let mut x = std::process::Command::new("cargo");
    x.arg("build");
    x.output().unwrap();
    let path = Path::new("D:\\projects\\wasmtime\\cranelift\\filetests\\filetests\\runtests");
    visit_dirs(path, &|f| {
        let filename = f.file_name().into_string().unwrap();
        if !filename.ends_with(".clif") {
            return;
        }
        if filename.contains("simd") {
            println!("skip, all simd not implemented,{}", filename);
            return;
        }
        let p = path.clone().join(filename);
        let x = runone::run(p.as_path(), None, Some("riscv64")).unwrap();

        parse_test(text, options);
        println!("end ---- {:?} used {:?}.", p, x);
    })
}
