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

static SIMPLE_LOGGER: SimpleLogger = SimpleLogger(log::Level::Debug);

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
    let s  : String = "
    alias.clif                                     i128-icmp-overflow.clif                        simd-icmp-of.clif
    arithmetic.clif                                i128-icmp.clif                                 simd-icmp-sge.clif
    atomic-cas-little.clif                         i128-load-store.clif                           simd-icmp-sgt.clif
    atomic-cas-subword-big.clif                    i128-reduce.clif                               simd-icmp-sle.clif
    atomic-cas-subword-little.clif                 i128-rotate.clif                               simd-icmp-slt.clif
    atomic-cas.clif                                i128-select.clif                               simd-icmp-uge.clif
    atomic-rmw-little.clif                         i128-shifts-small-types.clif                   simd-icmp-ugt.clif
    atomic-rmw-subword-big.clif                    i128-shifts.clif                               simd-icmp-ule.clif
    atomic-rmw-subword-little.clif                 iabs.clif                                      simd-icmp-ult.clif
    bextend.clif                                   iaddcarry.clif                                 simd-insertlane.clif
    bint.clif                                      iaddcin.clif                                   simd-lane-access.clif
    bitops.clif                                    iaddcout.clif                                  simd-logical.clif
    bitrev.clif                                    icmp-eq.clif                                   simd-saddsat-aarch64.clif
    bmask.clif                                     icmp-ne.clif                                   simd-saddsat.clif
    br.clif                                        icmp-nof.clif                                  simd-shuffle.clif
    br_icmp.clif                                   icmp-of.clif                                   simd-smulhi.clif
    br_icmp_overflow.clif                          icmp-sge.clif                                  simd-snarrow-aarch64.clif
    br_table.clif                                  icmp-sgt.clif                                  simd-snarrow.clif
    breduce.clif                                   icmp-sle.clif                                  simd-splat.clif
    cls-aarch64.clif                               icmp-slt.clif                                  simd-sqmulroundsat-aarch64.clif
    cls-interpret.clif                             icmp-uge.clif                                  simd-sqmulroundsat.clif
    clz-interpret.clif                             icmp-ugt.clif                                  simd-ssubsat-aarch64.clif
    clz.clif                                       icmp-ule.clif                                  simd-ssubsat.clif
    const.clif                                     icmp-ult.clif                                  simd-swidenhigh.clif
    ctz-interpret.clif                             icmp.clif                                      simd-swidenlow.clif
    ctz.clif                                       integer-minmax.clif                            simd-swizzle.clif
    div-checks.clif                                isubbin.clif                                   simd-uaddsat-aarch64.clif
    extend.clif                                    isubborrow.clif                                simd-uaddsat.clif
    fibonacci.clif                                 isubbout.clif                                  simd-umulhi.clif
    float-compare.clif                             load-op-store.clif                             simd-unarrow-aarch64.clif
    float.clif                                     popcnt-interpret.clif                          simd-unarrow.clif
    fmin-max-pseudo-vector.clif                    popcnt.clif                                    simd-usubsat-aarch64.clif
    fmin-max-pseudo.clif                           select.clif                                    simd-usubsat.clif
    heap.clif                                      shifts-small-types.clif                        simd-uunarrow.clif
    i128-arithmetic.clif                           shifts.clif                                    simd-uwidenhigh.clif
    i128-bandnot.clif                              simd-arithmetic-nondeterministic-aarch64.clif  simd-uwidenlow.clif
    i128-bextend.clif                              simd-arithmetic-nondeterministic-x86_64.clif   simd-valltrue.clif
    i128-bint.clif                                 simd-arithmetic.clif                           simd-vanytrue.clif
    i128-bitops-count.clif                         simd-bitselect-to-vselect.clif                 simd-vconst.clif
    i128-bitops.clif                               simd-bitselect.clif                            simd-vhighbits.clif
    i128-bitrev.clif                               simd-bitwise-run.clif                          simd-vselect.clif
    i128-bmask.clif                                simd-bitwise.clif                              simd-wideningpairwisedotproducts.clif
    i128-bornot.clif                               simd-bmask.clif                                simd_compare_zero.clif
    i128-br.clif                                   simd-comparison.clif                           smulhi-aarch64.clif
    i128-breduce.clif                              simd-conversion.clif                           smulhi.clif
    i128-bricmp.clif                               simd-extractlane.clif                          spill-reload.clif
    i128-bxornot.clif                              simd-iabs.clif                                 stack-addr-32.clif
    i128-cls.clif                                  simd-iaddpairwise.clif                         stack-addr-64.clif
    i128-concat-split.clif                         simd-icmp-eq.clif                              stack.clif
    i128-const.clif                                simd-icmp-ne.clif                              umulhi.clif
    i128-extend.clif                               simd-icmp-nof.clif                             xxx.clif
    ".into() ;

    let s: Vec<_> = s
        .split(" ")
        .map(|s| s.trim())
        .filter(|s| *s != "")
        .collect();
    let mut script = String::from("");

    for x in s {
        if x.contains("simd") {
            //todo:: remove when simd is supported.
            continue;
        }
        script = script +  format!("RUST_BACKTRACE=1 ./target/riscv64gc-unknown-linux-gnu/debug/clif-util   test  -d  --verbose   cranelift/filetests/filetests/runtests/{}\n", x).as_str();
        script += format!(
            r#"if [ "$?" != 0 ] ;then 
                    echo "{} failed."
                    exit;
                fi
        "#,
            x
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
    target riscv64gc has_extension_a
    
    
    
    function %i128_stack_store_load_big_offset() -> i32 {
    
    block0:
        v2 = iconst.i32 xxxxxxxxxxxxxxxx
        return v2
    }
    ; run: %i128_stack_store_load_big_offset() == xxxxxxxxxxxxxxxx
    
    "#
    .into();
    let mut rng = rand::thread_rng();
    for _ in 0..10000 {
        use std::io::Write;
        let file_name = "abc.clif";
        let file_content = s.replace("xxxxxxxxxxxxxxxx", format!("{}", rng.gen::<i32>()).as_str());
        let mut file = std::fs::File::create(file_name).expect("create failed");
        file.write_all(file_content.as_bytes())
            .expect("write failed");
        file.sync_all().unwrap();
        run_one_file(Path::new(file_name));
    }
}
