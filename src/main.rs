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
mod test_compile;

use log::{LevelFilter, Metadata, Record};

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
    let args: Vec<_> = std::env::args().collect();
    if args.len() > 1 {
        run_one_file(&Path::new(args[1].as_str()));
    } else {
        run_one_file(&Path::new("xxx.clif"));

        // run_one_file(&Path::new(
        //     "../wasmtime/cranelift/filetests/filetests/isa/riscv64/atomic_store.clif",
        // ));
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
        "compile" => test_compile::subtest(parsed),
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

fn pretty_anyhow_error(
    func: &cranelift_codegen::ir::Function,
    err: cranelift_codegen::CodegenError,
) -> anyhow::Error {
    let s = cranelift_codegen::print_errors::pretty_error(func, err);
    anyhow::anyhow!("{}", s)
}
