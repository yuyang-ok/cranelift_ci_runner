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
    {
        let mut x = std::process::Command::new("cargo");
        x.arg("build");

        let output = x.output().unwrap();
        println!("{}", String::from_utf8_lossy(&output.stdout[..]));
        println!("{}", String::from_utf8_lossy(&output.stdout[..]));
        if !output.status.success() {
            std::process::exit(output.status.code().unwrap());
        }
    }
    let run = || {
        let dir = String::from("../wasmtime/cranelift/filetests/filetests/runtests/");
        let files = vec![
            "alias.clif",
            "arithmetic.clif",
            "atomic-cas.clif",
            "bint.clif",
            "br_table.clif",
            "const.clif",
            "div-checks.clif",
            "i128-bint.clif",
            "i128-bitops.clif",
            // "i128-br.clif", not pass
            "i128-bornot.clif",
            "icmp-eq.clif",
            "icmp-ne.clif",
            "icmp-sge.clif",
            "icmp-sgt.clif",
            "icmp-sle.clif",
            "icmp-slt.clif",
            "icmp-uge.clif",
            "icmp-ugt.clif",
            "icmp-ule.clif",
            "icmp-ult.clif",
            "umulhi.clif",
            "i128-br.clif",
            "i128-bricmp.clif",
        ];
        let mut not_ok = vec![];
        for f in files {
            let mut cmd = std::process::Command::new("./target/debug/run_one");
            let mut path = dir.clone();
            path.push_str(f);
            cmd.arg(path.as_str());
            let output = cmd.output().unwrap();
            println!("{}", String::from_utf8_lossy(&output.stdout[..]));
            println!("{}", String::from_utf8_lossy(&output.stdout[..]));
            let code = output.status.code().unwrap();

            if code != 0 {
                println!("test no ok for {} , code : {}", f, code);
                // std::process::exit(code);
                not_ok.push(path.clone());
            }
        }
        println!("not oks{:?}", not_ok);
    };
    let out = || {
        visit_dirs(
            &Path::new("../wasmtime/cranelift/filetests/filetests/isa/riscv64"),
            &|entry: &DirEntry| {
                if entry.file_type().unwrap().is_dir() {
                    return;
                }
                if !entry.file_name().to_str().unwrap().ends_with(".clif") {
                    return;
                }
                println!("{:?}", &entry);
                let mut cmd = std::process::Command::new("./target/debug/run_one");
                cmd.arg(entry.path().to_str().unwrap());
                let output = cmd.output().unwrap();
                println!("{}", String::from_utf8_lossy(&output.stdout[..]));
                println!("{}", String::from_utf8_lossy(&output.stdout[..]));
                let code = output.status.code().unwrap();

                if code != 0 {
                    println!(
                        "test no ok for {:?} , code : {}",
                        entry.path().to_str(),
                        code
                    );
                    // std::process::exit(code);
                }
            },
        )
        .unwrap();
    };
     out();
    run();
}
