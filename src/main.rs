mod compiler;
mod sub_test;
use sub_test::*;
pub mod runone;
use cranelift_reader::TestCommand;
mod runtest_environment;
mod test_run;
use cranelift_codegen::isa::lookup_by_name;
use cranelift_codegen::{ir, isa::TargetIsa};
use std::{borrow::Cow, path::Path};

mod call;
mod test_compile;

use log::{LevelFilter, Metadata, Record};

struct SimpleLogger(log::Level);

static SIMPLE_LOGGER: SimpleLogger = SimpleLogger(log::Level::Debug);

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() >= self.0
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
        //     "../wasmtime/cranelift/filetests/filetests/runtests/alias.clif",
        // ));
        // run_one_file(&Path::new(
        //     "../wasmtime/cranelift/filetests/filetests/runtests/arithmetic.clif",
        // ));
        // run_one_file(&Path::new(
        //     "../wasmtime/cranelift/filetests/filetests/runtests/atomic-cas.clif",
        // ));

        // run_one_file(&Path::new(
        //     "../wasmtime/cranelift/filetests/filetests/isa/riscv64/atomic_store.clif",
        // ));
        // run_one_file(&Path::new(
        //     "../wasmtime/cranelift/filetests/filetests/isa/riscv64/condbr.clif",
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

pub fn build_backend() -> Box<dyn TargetIsa> {
    let builder = lookup_by_name("riscv64").unwrap();
    let shared_builder = cranelift_codegen::settings::builder();
    let shared_flags = cranelift_codegen::settings::Flags::new(shared_builder);
    let isa = builder.finish(shared_flags).unwrap();
    isa
}
