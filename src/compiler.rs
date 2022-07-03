use anyhow::Result;
use core::mem;
use cranelift_codegen::data_value::DataValue;
use cranelift_codegen::ir::{condcodes::IntCC, Function, InstBuilder, Signature};
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
    isa: Box<dyn TargetIsa>,
    trampolines: HashMap<Signature, Trampoline>,
}

impl SingleFunctionCompiler {
    /// Build a [SingleFunctionCompiler] from a [TargetIsa]. For functions to be runnable on the
    /// host machine, this [TargetIsa] must match the host machine's ISA (see
    /// [SingleFunctionCompiler::with_host_isa]).
    pub fn new(isa: Box<dyn TargetIsa>) -> Self {
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
        let code_page = compile2(function, self.isa.as_ref())?.0;

        // Compile the trampoline to call it, if necessary (it may be cached).

        let trampoline = self
            .trampolines
            .entry(signature.clone())
            .or_insert_with(|| {
                let ir = make_trampoline(&signature, self.isa.as_ref());
                let code = compile2(ir, self.isa.as_ref())
                    .expect("failed to compile trampoline")
                    .0;
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

    pub fn call2(&self, arguments: &[DataValue]) -> Vec<DataValue> {
        use rvemu::bus::DRAM_BASE;
        use rvemu::cpu::DOUBLEWORD;
        use rvemu::emulator::Emulator;

        {
            use std::io::Write;
            let mut file = std::fs::File::create("code.bin").unwrap();
            file.write_all(&self.code[..]).unwrap();
            let mut file = std::fs::File::create("trampoline.bin").unwrap();
            file.write_all(&self.trampoline.code[..]).unwrap();
        }

        let mut values = UnboxedValues::make_arguments(arguments, &self.signature);
        let mut dram = Vec::new();
        dram.extend(self.trampoline.data());
        //
        let end_at = (dram.len() as u64) + rvemu::bus::DRAM_BASE - 4;
        let func_addr = (dram.len() as u64) + rvemu::bus::DRAM_BASE;
        dram.extend(&self.code[..]);
        let arguments_area = (dram.len() as u64) + rvemu::bus::DRAM_BASE;
        let mut args = values.make_vec_u8();
        dram.extend(&args[..]);

        let mut emulator = Emulator::new();
        emulator.initialize_dram(dram);

        // write
        emulator.cpu.xregs.write(10, func_addr);

        emulator.cpu.xregs.write(11, arguments_area);
        emulator.initialize_pc(rvemu::bus::DRAM_BASE);
        emulator.test_run_end_at(end_at).unwrap();
        for k in 0..self.signature.returns.len() {
            let addr = arguments_area + (k as u64) * 16;
            let v1 = emulator.cpu.bus.read(addr, DOUBLEWORD).unwrap() as u128;
            let v2 = emulator.cpu.bus.read(addr + 8, DOUBLEWORD).unwrap() as u128;
            let v: u128 = v1 | v2 << 64;
            values.0[k] = v;
            println!("#############{:?}", v);
        }

        let result = values.collect_returns(&self.signature);
        println!("!!!!!!!!!!!!!!!!!!!!!{:?}", result);
        return result;
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

fn compile2(
    function: Function,
    isa: &dyn TargetIsa,
) -> Result<(Vec<u8>, String), CompilationError> {
    // Compile and encode the result to machine code.
    use cranelift_codegen::Context;
    let mut c = Context::for_function(function);
    c.want_disasm = true;
    c.compile(isa).unwrap();

    let result = c.mach_compile_result.unwrap();
    Ok((
        Vec::from_iter(result.buffer.data().iter().map(|v| *v)),
        result.disasm.unwrap(),
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
                let v = builder.ins().icmp_imm(IntCC::NotEqual, loaded, 0);
                if param.value_type.bits() > 1 {
                    builder.ins().bextend(param.value_type, v)
                } else {
                    v
                }
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
