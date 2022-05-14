mod compiler;
mod sub_test;
use sub_test::*;
pub mod runone;
use cranelift_reader::TestCommand;
mod runtest_environment;
mod test_run;
use cranelift_codegen::ir;
use std::borrow::Cow;
use std::path::Path;

use log::{Level, LevelFilter, Metadata, Record};

use walkdir;

struct SimpleLogger;

static SIMPLE_LOGGER: SimpleLogger = SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
    fn flush(&self) {}
}

fn init_logger() {
    log::set_logger(&SIMPLE_LOGGER)
        .map(|()| log::set_max_level(LevelFilter::max()))
        .unwrap()
}

fn main() {
    init_logger();
    // let path = Path::new("D:\\projects\\wasmtime\\cranelift\\filetests\\filetests\\runtests");
    // visit_dirs(path, &|f| {
    //     let filename = f.file_name().into_string().unwrap();
    //     if !filename.ends_with(".clif") {
    //         return;
    //     }
    //     if filename.contains("simd") {
    //         println!("skip, all simd not implemented,{}", filename);
    //         return;
    //     }
    //     let p = path.clone().join(filename);
    //     let x = runone::run(p.as_path(), None, Some("riscv64")).unwrap();
    //     println!("end ---- {:?} used {:?}.", p, x);
    // })
    // .unwrap();
    // let args: Vec<String> = std::env::args().collect();
    // let p = Path::new(args[1].as_str());
    // let x = runone::run(&p, None, Some("riscv64")).unwrap();
    // println!("end ---- {:?} used {:?}.", p, x);

    //

    // run_one_file(&Path::new(
    //     "D:\\projects\\wasmtime\\cranelift\\filetests\\filetests\\runtests\\alias.clif",
    // ));
    // run_one_file(&Path::new(
    //     "D:\\projects\\wasmtime\\cranelift\\filetests\\filetests\\runtests\\arithmetic.clif",
    // ));

    // run_one_file(&Path::new(
    //     "D:\\projects\\wasmtime\\cranelift\\filetests\\filetests\\runtests\\atomic-cas-subword-little.clif",
    // ));
    // run_one_file(&Path::new(
    //     "D:\\projects\\wasmtime\\cranelift\\filetests\\filetests\\runtests\\",
    // ));
    let args: Vec<_> = std::env::args().collect();
    if args.len() >= 1 {
        run_one_file(&Path::new(args[1].as_str()));
    } else {
        run_one_file(&Path::new("xxx.clif"));
    }
}

fn run_one_file(p: &Path) {
    let x = runone::run(&p, None, None).unwrap();
    println!("{:?} {:?}", p, x);
}

/// Create a new subcommand trait object to match `parsed.command`.
///
/// This function knows how to create all of the possible `test <foo>` commands that can appear in
/// a `.clif` test file.
fn new_subtest(parsed: &TestCommand) -> anyhow::Result<Box<dyn SubTest>> {
    match parsed.command {
        "run" => test_run::subtest(parsed),
        _ => subskip(parsed),
    }
}

struct TestSkip;

pub fn subskip(parsed: &TestCommand) -> anyhow::Result<Box<dyn SubTest>> {
    Ok(Box::new(TestSkip))
}

impl SubTest for TestSkip {
    fn name(&self) -> &'static str {
        "false test"
    }

    fn is_mutating(&self) -> bool {
        false
    }

    fn needs_isa(&self) -> bool {
        false
    }

    fn run(&self, func: Cow<ir::Function>, context: &Context) -> anyhow::Result<()> {
        Ok(())
    }
}
