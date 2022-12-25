mod compiler;
mod sub_test;
use rand::Rng;
use sub_test::*;
pub mod runone;
use cranelift_reader::TestCommand;
mod runtest_environment;
mod test_run;
use cranelift_codegen::isa::lookup_by_name;
use cranelift_codegen::{ir, isa::TargetIsa};
use std::fmt::format;
use std::{borrow::Cow, path::Path};

mod call;
mod interpreter;
mod test_compile;
use log::{LevelFilter, Metadata, Record};

struct SimpleLogger(log::Level);

static SIMPLE_LOGGER: SimpleLogger = SimpleLogger(log::Level::Trace);

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
        .map(|()| log::set_max_level(LevelFilter::Trace))
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
        // run_one_file(&Path::new(
        //     "../wasmtime/cranelift/filetests/filetests/isa/riscv64/condops.clif",
        // ));
        // run_one_file(&Path::new(
        //     "../wasmtime/cranelift/filetests/filetests/isa/riscv64/uextend-sextend.clif",
        // ));
    }
}

fn run_one_file(p: &Path) {
    let x = runone::run(&p, None, None).unwrap();
    println!("##################{:?} {:?}", p, x);
}

/// Create a new subcommand trait object to match `parsed.command`.
///
/// This function knows how to create all of the possible `test <foo>` commands that can appear in
/// a `.clif` test file.
fn new_subtest(parsed: &TestCommand) -> anyhow::Result<Box<dyn SubTest>> {
    match parsed.command {
        "run" => test_run::subtest(parsed),
        "compile" => test_compile::subtest(parsed),
        "interpret" => interpreter::subtest(parsed),
        _ => subskip(parsed),
    }
}

struct TestSkip;

pub fn subskip(_: &TestCommand) -> anyhow::Result<Box<dyn SubTest>> {
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
    let builder = lookup_by_name("riscv64gc").unwrap();
    let shared_builder = cranelift_codegen::settings::builder();
    let shared_flags = cranelift_codegen::settings::Flags::new(shared_builder);
    let isa = builder.finish(shared_flags).unwrap();
    isa
}

#[test]
fn one_by_one_run_sh() {
    let s: Vec<_> =
        std::fs::read_dir("/home/yuyang/projects/wasmtime/cranelift/filetests/filetests/runtests")
            .unwrap()
            .into_iter()
            .map(|r| r.unwrap().file_name())
            .collect();

    let mut script = String::from("");

    for x in s {
        if x.to_str().unwrap().contains("simd") {
            //todo:: remove when simd is supported.
            continue;
        }
        script = script +  format!("/home/yuyang/projects/qemu/build/qemu-riscv64 -L /usr/riscv64-linux-gnu -E LD_LIBRARY_PATH=/usr/riscv64-linux-gnu/lib -E RUST_BACKTRACE=0  -E RUST_LOG=error    ./target/riscv64gc-unknown-linux-gnu/debug/clif-util   test  --verbose   cranelift/filetests/filetests/runtests/{}\n",       x.to_str().unwrap()).as_str();
        script += format!(
            r#"if [ "$?" != 0 ] ;then 
                    echo "{} failed."
                    exit;
                fi
        "#,
            x.to_str().unwrap()
        )
        .as_str();
    }
    use std::io::Write;
    let mut file = std::fs::File::create("run_one_by_one.sh").expect("create failed");
    file.write_all(script.as_bytes()).expect("write failed");
}

#[test]
fn xxx() {
    let s: String = r#"
    
    test run
    target s390x
    target aarch64
    target aarch64 has_lse
    target x86_64
    target riscv64
    
    
    
    function %i128_stack_store_load_big_offset() -> i64 {
    
    block0:
        v2 = iconst.i64 xxxxxxxxxxxxxxxx
        return v2
    }
    ; run: %i128_stack_store_load_big_offset() == xxxxxxxxxxxxxxxx
    
    "#
    .into();
    let mut rng = rand::thread_rng();
    for _ in 0..10000 {
        use std::io::Write;
        let file_name = "abc.clif";
        let file_content = s.replace("xxxxxxxxxxxxxxxx", format!("{}", rng.gen::<i64>()).as_str());
        let mut file = std::fs::File::create(file_name).expect("create failed");
        file.write_all(file_content.as_bytes())
            .expect("write failed");
        file.sync_all().unwrap();
        run_one_file(Path::new(file_name));
    }
}

#[test]
fn fdsfsfsdf() {
    fn xxx(name: &str, f: fn() -> [bool; 32]) {
        let x = f();
        println!("const {} : [bool; 32]= [", name);
        for i in 0..=31 {
            print!("{}{}", x[i], if i != 31 { "," } else { "" });
            if (i + 1) % 4 == 0 {
                println!("// {}-{}", i - 3, i);
            }
        }
        println!("];");
    }
    xxx("CALLER_SAVE_X_REG", get_caller_save_x_gpr);
    xxx("CALLEE_SAVE_X_REG", get_callee_save_x_gpr);
    xxx("CALLER_SAVE_F_REG", get_caller_save_f_gpr);
    xxx("CALLEE_SAVE_F_REG", get_callee_save_f_gpr);
}

fn get_caller_save_x_gpr() -> [bool; 32] {
    let mut x: [bool; 32] = [false; 32];
    for (i, v) in get_callee_save_x_gpr().iter().enumerate() {
        if i == 0 || i == 3 || i == 4 {
            continue;
        }
        x[i] = !v;
    }
    x
}

fn get_caller_save_f_gpr() -> [bool; 32] {
    let mut x: [bool; 32] = [false; 32];
    for (i, v) in get_callee_save_f_gpr().iter().enumerate() {
        x[i] = !v;
    }
    x
}

fn get_callee_save_x_gpr() -> [bool; 32] {
    let mut x = [false; 32];
    x[2] = true;
    for i in 8..=9 {
        x[i] = true
    }
    for i in 18..=27 {
        x[i] = true
    }
    x
}

fn get_callee_save_f_gpr() -> [bool; 32] {
    let mut x = [false; 32];
    for i in 8..9 {
        x[i] = true;
    }
    for i in 18..=27 {
        x[i] = true
    }
    x
}

// #[test]
// fn dfdsfsdfs() {
//     println!( " {}  {} " ，  )
// }
