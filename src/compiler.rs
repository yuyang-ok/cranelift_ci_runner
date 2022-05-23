use anyhow::Result;
use core::mem;
use cranelift_codegen::data_value::DataValue;
use cranelift_codegen::ir::{condcodes::IntCC, Function, InstBuilder, Signature};
use cranelift_codegen::isa::riscv64::Riscv64Backend;
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::{ir, settings, CodegenError, Context};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_native::builder_with_options;
use log::trace;
use memmap2::{Mmap, MmapMut};
use std::cmp::max;
use std::collections::HashMap;
use std::process::Output;
use std::thread::current;
use thiserror::Error;

use unicorn_engine::unicorn_const::{Arch, Mode, Permission};
use unicorn_engine::{RegisterRISCV, Unicorn};
/// Compile a single function.
///
/// Several Cranelift functions need the ability to run Cranelift IR (e.g. `test_run`); this
/// [SingleFunctionCompiler] provides a way for compiling Cranelift [Function]s to
/// `CompiledFunction`s and subsequently calling them through the use of a `Trampoline`. As its
/// name indicates, this compiler is limited: any functionality that requires knowledge of things
/// outside the [Function] will likely not work (e.g. global values, calls). For an example of this
/// "outside-of-function" functionality, see `cranelift_jit::backend::JITBackend`.
///
/// ```
/// use cranelift_filetests::SingleFunctionCompiler;
/// use cranelift_reader::parse_functions;
///
/// let code = "test run \n function %add(i32, i32) -> i32 {  block0(v0:i32, v1:i32):  v2 = iadd v0, v1  return v2 }".into();
/// let func = parse_functions(code).unwrap().into_iter().nth(0).unwrap();
/// let mut compiler = SingleFunctionCompiler::with_default_host_isa().unwrap();
/// let compiled_func = compiler.compile(func).unwrap();
/// println!("Address of compiled function: {:p}", compiled_func.as_ptr());
/// ```
pub struct SingleFunctionCompiler {
    isa: Riscv64Backend,
    trampolines: HashMap<Signature, Trampoline>,
}

impl SingleFunctionCompiler {
    /// Build a [SingleFunctionCompiler] from a [TargetIsa]. For functions to be runnable on the
    /// host machine, this [TargetIsa] must match the host machine's ISA (see
    /// [SingleFunctionCompiler::with_host_isa]).
    pub fn new(isa: Riscv64Backend) -> Self {
        let trampolines = HashMap::new();
        Self { isa, trampolines }
    }

    /// Compile the passed [Function] to a `CompiledFunction`. This function will:
    ///  - check that the default ISA calling convention is used (to ensure it can be called)
    ///  - compile the [Function]
    ///  - compile a `Trampoline` for the [Function]'s signature (or used a cached `Trampoline`;
    ///    this makes it possible to call functions when the signature is not known until runtime.
    pub fn compile(&mut self, function: Function) -> Result<CompiledFunction, CompilationError> {
        let signature = function.signature.clone();

        // Compile the function itself.
        let code_page = compile2(function, &self.isa)?;

        // Compile the trampoline to call it, if necessary (it may be cached).
        let isa = &self.isa;
        let trampoline = self
            .trampolines
            .entry(signature.clone())
            .or_insert_with(|| {
                let ir = make_trampoline(&signature, isa);
                let code = compile2(ir, isa).expect("failed to compile trampoline");
                Trampoline::new(code)
            });

        Ok(CompiledFunction::new(code_page, signature, trampoline))
    }
}

/// Compilation Error when compiling a function.
#[derive(Error, Debug)]
pub enum CompilationError {
    /// This Target ISA is invalid for the current host.
    #[error("Cross-compilation not currently supported; use the host's default calling convention \
    or remove the specified calling convention in the function signature to use the host's default.")]
    InvalidTargetIsa,
    /// Cranelift codegen error.
    #[error("Cranelift codegen error")]
    CodegenError(#[from] CodegenError),
    /// Memory mapping error.
    #[error("Memory mapping error")]
    IoError(#[from] std::io::Error),
}

/// Contains the compiled code to move memory-allocated [DataValue]s to the correct location (e.g.
/// register, stack) dictated by the calling convention before calling a [CompiledFunction]. Without
/// this, it would be quite difficult to correctly place [DataValue]s since both the calling
/// convention and function signature are not known until runtime. See [make_trampoline] for the
/// Cranelift IR used to build this.
pub struct Trampoline {
    code: Vec<u8>,
}

impl Trampoline {
    /// Build a new [Trampoline].
    pub fn new(page: Vec<u8>) -> Self {
        Self { code: page }
    }
    fn len(self) -> usize {
        self.code.len()
    }
    fn data(&self) -> &[u8] {
        &self.code[..]
    }
}

/// Container for the compiled code of a [Function]. This wrapper allows users to call the compiled
/// function through the use of a [Trampoline].
///
/// ```
/// use cranelift_filetests::SingleFunctionCompiler;
/// use cranelift_reader::parse_functions;
/// use cranelift_codegen::data_value::DataValue;
///
/// let code = "test run \n function %add(i32, i32) -> i32 {  block0(v0:i32, v1:i32):  v2 = iadd v0, v1  return v2 }".into();
/// let func = parse_functions(code).unwrap().into_iter().nth(0).unwrap();
/// let mut compiler = SingleFunctionCompiler::with_default_host_isa().unwrap();
/// let compiled_func = compiler.compile(func).unwrap();
///
/// let returned = compiled_func.call(&vec![DataValue::I32(2), DataValue::I32(40)]);
/// assert_eq!(vec![DataValue::I32(42)], returned);
/// ```
pub struct CompiledFunction<'a> {
    code: Vec<u8>,
    signature: Signature,
    trampoline: &'a Trampoline,
}

/*
    notice!!! register number begin with 1
*/
impl<'a> CompiledFunction<'a> {
    /// Build a new [CompiledFunction].
    pub fn new(page: Vec<u8>, signature: Signature, trampoline: &'a Trampoline) -> Self {
        Self {
            code: page,
            signature,
            trampoline,
        }
    }

    // fn call2() -> Vec<DataValue> {
    //     {
    //         use std::io::Write;
    //         let mut file = std::fs::File::create("d://code.bin").unwrap();
    //         file.write_all(&self.code[..]).unwrap();
    //         let mut file = std::fs::File::create("d://trampoline.bin").unwrap();
    //         file.write_all(&self.trampoline.code[..]).unwrap();
    //     }
    //     use rvemu::emulator::Emulator;
    //     let mut values = UnboxedValues::make_arguments(arguments, &self.signature);
    //     let emulator = Emulator::new();

    //     emulator.initialize_dram();
    //     emulator.start();
    // }

    pub fn call(&self, arguments: &[DataValue]) -> Vec<DataValue> {
        {
            use std::io::Write;
            let mut file = std::fs::File::create("d://code.bin").unwrap();
            file.write_all(&self.code[..]).unwrap();
            let mut file = std::fs::File::create("d://trampoline.bin").unwrap();
            file.write_all(&self.trampoline.code[..]).unwrap();
        }
        let mut values = UnboxedValues::make_arguments(arguments, &self.signature);
        let mut emulator = Unicorn::new(Arch::RISCV, Mode::RISCV64).unwrap();

        let mut cur_memory_address = 0x1000 as u64;
        let k_4: usize = 1024 * 4;

        assert!(self.trampoline.data().len() < k_4);
        assert!(self.code.len() < k_4);

        let program_start = cur_memory_address;
        emulator
            .mem_map(cur_memory_address, k_4, Permission::all())
            .unwrap();

        emulator
            .mem_write(cur_memory_address, self.trampoline.data())
            .unwrap();
        cur_memory_address += k_4 as u64;

        // my code
        let function_addr = cur_memory_address;
        emulator
            .mem_map(cur_memory_address, k_4, Permission::ALL)
            .unwrap();
        emulator
            .mem_write(cur_memory_address, &self.code[..])
            .unwrap();
        cur_memory_address += k_4 as u64;
        let program_end = program_start + self.trampoline.data().len() as u64 - 4;
        // arguments
        let mut arguments_data = values.make_vec_u8();
        assert!(arguments_data.len() < k_4);
        let argument_addr = cur_memory_address;
        emulator
            .mem_map(cur_memory_address, k_4, Permission::ALL)
            .unwrap();
        emulator
            .mem_write(cur_memory_address, &arguments_data[..])
            .unwrap();
        cur_memory_address += k_4 as u64;

        //paramter a0 and a1
        emulator
            .reg_write(RegisterRISCV::X10, function_addr)
            .unwrap();
        emulator
            .reg_write(RegisterRISCV::X11, argument_addr)
            .unwrap();
        // sp
        let sp_start = 2 * 1024 * 1024;
        let extra_sp_size = 4 * 1024;
        let sp_end = sp_start + 1024 * 1024; /*extra sp size*/
        let reg_sp = RegisterRISCV::X2;
        emulator.reg_write(reg_sp, sp_end).unwrap();
        // map sp
        emulator
            .mem_map(
                sp_start,
                ((sp_end - sp_start) + extra_sp_size/* 假设有人在调用我 多余的1m是他的空间*/)
                    as usize,
                Permission::all(),
            )
            .unwrap();
        // emulator.mem_write(sp_start, &[0; 1024 * 1024][..]);
        emulator
            .add_code_hook(
                program_start,
                program_start + self.trampoline.data().len() as u64,
                |e, pc, _code_length| {
                    let mut data = [0; 4];
                    e.mem_read(pc, &mut data[..]).unwrap();
                    println!("trampoline:{}", Self::dis(&data[..]));
                },
            )
            .unwrap();

        emulator
            .add_code_hook(
                function_addr,
                function_addr + self.code.len() as u64,
                |e, pc, _code_length| {
                    if pc == function_addr {
                        let print_arg = |name: &str, r: i32| {
                            let value = e.reg_read(r).unwrap();
                            println!("{}_u64:{},{}_i64:{}", name, value, name, value as i64)
                        };
                        for i in 0..=7 {
                            print_arg(format!("a{}", i).as_str(), (RegisterRISCV::X10 as i32) + i);
                        }
                    }

                    let mut data = [0; 4];
                    e.mem_read(pc, &mut data[..]).unwrap();
                    println!("test_function:{}", Self::dis(&data[..]));
                },
            )
            .unwrap();

        let x = emulator.emu_start(
            program_start,
            program_start + self.trampoline.data().len() as u64 - 4,
            0,
            0,
        );

        match x {
            Ok(_) => {}
            Err(err) => {
                println!("err : {:?}", err);
                // print alot of status of emulator.
                {
                    // pc
                    let pc = emulator.pc_read().unwrap();
                    let pc_in_code_range = || -> bool {
                        pc >= function_addr && pc <= function_addr + self.code.len() as u64 - 4
                    };
                    let pc_trampline_range =
                        || -> bool { pc >= program_start && pc <= program_end };
                    if pc_in_code_range() {
                        println!("pc in code range..")
                    } else if pc_trampline_range() {
                        println!("pc in trampline range..")
                    } else {
                        println!("pc({}) is out of range.", pc);
                    }
                }

                let check_addr_in_sp_range = |addr: u64| {
                    // sp
                    println!(
                        "addr {}  sp-sp_start={} sp_end-sp={}",
                        addr,
                        addr - sp_start,
                        sp_end - addr
                    );
                    let sp_in_range = || addr >= sp_start && addr <= sp_end - 8;
                    if sp_in_range() {
                        println!("addr in range")
                    } else if addr < sp_start {
                        println!("addr is less than sp_start {}", sp_start - addr);
                    } else {
                        println!("addr is greater than sp_end {}", addr - sp_end);
                    }
                };
                println!("check is sp in stack range");
                check_addr_in_sp_range(emulator.reg_read(reg_sp).unwrap());
                println!("check is sp in t2 range");
                check_addr_in_sp_range(emulator.reg_read(RegisterRISCV::X7).unwrap());
                x.unwrap()
            }
        }

        // read back
        emulator
            .mem_read(argument_addr, &mut arguments_data[..])
            .unwrap();

        // write to origin vector
        values.wirte_vec_u8_back(&arguments_data);
        println!("result{:?}", values.0,);
        emulator.emu_stop();
        drop(emulator);
        values.collect_returns(&self.signature)
    }

    pub fn x_reg_name(num: i32) -> String {
        match num {
            0 => "zero".into(),
            1 => "ra".into(),
            2 => "sp".into(),
            3 => "gp".into(),
            4 => "tp".into(),
            5 => "t0".into(),
            6..=7 => format!("t{}", num - 5),
            8 => "fp".into(),
            9 => "s1".into(),
            10..=17 => format!("a{}", num - 10),
            18..=27 => format!("s{}", num - 16),
            28..=31 => format!("t{}", num - 25),
            _ => unreachable!(),
        }
    }
    pub fn f_reg_name(num: i32) -> String {
        match num {
            0..=7 => format!("ft{}", num - 0),
            8..=9 => format!("fs{}", num - 8),
            10..=17 => format!("fa{}", num - 10),
            18..=27 => format!("fs{}", num - 16),
            28..=31 => format!("ft{}", num - 20),
            _ => unreachable!(),
        }
    }

    fn dis(data: &[u8]) -> String {
        let file_name = "d://one_instruction.bin";
        use std::io::Write;
        let mut file = std::fs::File::create(file_name).unwrap();
        file.write_all(data).unwrap();
        file.sync_all().unwrap();
        use std::process::Command;
        /* */
        let mut cmd = Command::new("riscv64-unknown-elf-objdump");
        let cmd = cmd.args(&["-b", "binary", "-m", "riscv:rv64", "-D", file_name]);
        let output = cmd.output().expect("exec objdump failed , {}");
        // println!(
        //     "!!!!!!!!!!!!!!!!!!!{} {}",
        //     String::from_utf8_lossy(&output.stderr[..]),
        //     String::from_utf8_lossy(&output.stdout[..])
        // );
        let output = output.stdout;
        /*
            a.out:     file format elf64-littleriscv

        Disassembly of section .text:

        0000000000000000 <.text>:
           0:   fe010113                addi    sp,sp,-32
            */
        let mut ret = String::default();
        let mut i = 0;
        while i < output.len() {
            // match   0:
            let mut _match = || -> bool {
                if output[i] == ('0' as u8) && output[i + 1] == (':' as u8) {
                    i += 2;
                    true
                } else {
                    false
                }
            };
            if _match() {
                while i < output.len() {
                    if output[i] == 10 {
                        break;
                    }
                    ret.push(output[i] as char);
                    i += 1;
                }
            }
            i += 1;
        }
        ret
    }

    fn print_emulator_statue<D>(emulator: &unicorn_engine::Unicorn<D>) {
        for i in 1..=32 {
            print!(
                "{:<5}{:<10}",
                format!("{}", Self::x_reg_name(i - 1)), /* start from zero */
                format!("({})", emulator.reg_read(i).unwrap())
            );
            if i == 32 {
                // print extra pc
                print!("pc:{}", emulator.reg_read(RegisterRISCV::PC).unwrap());
            }
            if i % 8 == 0 {
                println!()
            }
        }
        println!("\n")
    }

    //
}

/// A container for laying out the [ValueData]s in memory in a way that the [Trampoline] can
/// understand.
struct UnboxedValues(Vec<u128>);

impl UnboxedValues {
    /// The size in bytes of each slot location in the allocated [DataValue]s. Though [DataValue]s
    /// could be smaller than 16 bytes (e.g. `I16`), this simplifies the creation of the [DataValue]
    /// array and could be used to align the slots to the largest used [DataValue] (i.e. 128-bit
    /// vectors).
    const SLOT_SIZE: usize = 16;

    /// Build the arguments vector for passing the [DataValue]s into the [Trampoline]. The size of
    /// `u128` used here must match [Trampoline::SLOT_SIZE].
    pub fn make_arguments(arguments: &[DataValue], signature: &ir::Signature) -> Self {
        assert_eq!(arguments.len(), signature.params.len());
        let mut values_vec = vec![0; max(signature.params.len(), signature.returns.len())];
        // Store the argument values into `values_vec`.
        for ((arg, slot), param) in arguments.iter().zip(&mut values_vec).zip(&signature.params) {
            assert!(
                arg.ty() == param.value_type || arg.is_vector() || arg.is_bool(),
                "argument type mismatch: {} != {}",
                arg.ty(),
                param.value_type
            );
            unsafe {
                arg.write_value_to(slot);
            }
        }

        Self(values_vec)
    }

    /// Collect the returned [DataValue]s into a [Vec]. The size of `u128` used here must match
    /// [Trampoline::SLOT_SIZE].
    pub fn collect_returns(&self, signature: &ir::Signature) -> Vec<DataValue> {
        assert!(self.0.len() >= signature.returns.len());
        let mut returns = Vec::with_capacity(signature.returns.len());

        // Extract the returned values from this vector.
        for (slot, param) in self.0.iter().zip(&signature.returns) {
            let value = unsafe { DataValue::read_value_from(slot, param.value_type) };
            returns.push(value);
        }
        returns
    }
    pub fn make_vec_u8(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.0.len() * 16);
        for data in self.0.iter() {
            v.extend(data.to_le_bytes().into_iter());
        }
        v
    }

    fn wirte_vec_u8_back(&mut self, data: &Vec<u8>) {
        let mut step = 0;
        while step < data.len() {
            let mut u128_tmp_data = [0 as u8; 16];
            for i in 0..16 {
                u128_tmp_data[(i + step) % 16] = data[step + i];
            }
            self.0[step / 16] = u128::from_le_bytes(u128_tmp_data);
            step += 16;
        }
    }
}

/// Compile a [Function] to its executable bytes in memory.
///
/// This currently returns a [Mmap], a type from an external crate, so we wrap this up before
/// exposing it in public APIs.
// fn compile(function: Function, isa: &Riscv64Backend) -> Result<Vec<u8>, CompilationError> {
//     // Compile and encode the result to machine code.
//     let code_info = isa.compile_function_test(&function, true).unwrap();
//     Ok(Vec::from_iter(code_info.buffer.data().iter().map(|v| *v)))
// }

fn compile2(function: Function, isa: &Riscv64Backend) -> Result<Vec<u8>, CompilationError> {
    // Compile and encode the result to machine code.
    use cranelift_codegen::Context;
    let mut c = Context::for_function(function);
    c.want_disasm = true;
    c.compile(isa).unwrap();
    Ok(Vec::from_iter(
        c.mach_compile_result
            .unwrap()
            .buffer
            .data()
            .iter()
            .map(|v| *v),
    ))
}

/// Build the Cranelift IR for moving the memory-allocated [DataValue]s to their correct location
/// (e.g. register, stack) prior to calling a [CompiledFunction]. The [Function] returned by
/// [make_trampoline] is compiled to a [Trampoline]. Note that this uses the [TargetIsa]'s default
/// calling convention so we must also check that the [CompiledFunction] has the same calling
/// convention (see [SingleFunctionCompiler::compile]).
fn make_trampoline(signature: &ir::Signature, isa: &dyn TargetIsa) -> Function {
    // Create the trampoline signature: (callee_address: pointer, values_vec: pointer) -> ()
    let pointer_type = isa.pointer_type();
    let mut wrapper_sig = ir::Signature::new(isa.frontend_config().default_call_conv);
    wrapper_sig.params.push(ir::AbiParam::new(pointer_type)); // Add the `callee_address` parameter.
    wrapper_sig.params.push(ir::AbiParam::new(pointer_type)); // Add the `values_vec` parameter.

    let mut func = ir::Function::with_name_signature(ir::ExternalName::user(0, 0), wrapper_sig);

    // The trampoline has a single block filled with loads, one call to callee_address, and some loads.
    let mut builder_context = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut func, &mut builder_context);
    let block0 = builder.create_block();
    builder.append_block_params_for_function_params(block0);
    builder.switch_to_block(block0);
    builder.seal_block(block0);

    // Extract the incoming SSA values.
    let (callee_value, values_vec_ptr_val) = {
        let params = builder.func.dfg.block_params(block0);
        (params[0], params[1])
    };

    // Load the argument values out of `values_vec`.
    let callee_args = signature
        .params
        .iter()
        .enumerate()
        .map(|(i, param)| {
            // Calculate the type to load from memory, using integers for booleans (no encodings).
            let ty = param.value_type.coerce_bools_to_ints();

            // Load the value.
            let loaded = builder.ins().load(
                ty,
                ir::MemFlags::trusted(),
                values_vec_ptr_val,
                (i * UnboxedValues::SLOT_SIZE) as i32,
            );

            // For booleans, we want to type-convert the loaded integer into a boolean and ensure
            // that we are using the architecture's canonical boolean representation (presumably
            // comparison will emit this).
            if param.value_type.is_bool() {
                builder.ins().icmp_imm(IntCC::NotEqual, loaded, 0)
            } else if param.value_type.is_bool_vector() {
                let zero_constant = builder.func.dfg.constants.insert(vec![0; 16].into());
                let zero_vec = builder.ins().vconst(ty, zero_constant);
                builder.ins().icmp(IntCC::NotEqual, loaded, zero_vec)
            } else {
                loaded
            }
        })
        .collect::<Vec<_>>();

    // Call the passed function.
    let new_sig = builder.import_signature(signature.clone());
    let call = builder
        .ins()
        .call_indirect(new_sig, callee_value, &callee_args);

    // Store the return values into `values_vec`.
    let results = builder.func.dfg.inst_results(call).to_vec();
    for ((i, value), param) in results.iter().enumerate().zip(&signature.returns) {
        // Before storing return values, we convert booleans to their integer representation.
        let value = if param.value_type.lane_type().is_bool() {
            let ty = param.value_type.lane_type().as_int();
            builder.ins().bint(ty, *value)
        } else {
            *value
        };
        // Store the value.
        builder.ins().store(
            ir::MemFlags::trusted(),
            value,
            values_vec_ptr_val,
            (i * UnboxedValues::SLOT_SIZE) as i32,
        );
    }
    builder.ins().return_(&[]);
    builder.finalize();
    func
}
