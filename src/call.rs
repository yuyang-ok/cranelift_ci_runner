use crate::compiler::CompiledFunction;

// impl CompiledFunction {
//     pub fn call(&self, arguments: &[DataValue]) -> Vec<DataValue> {
//         {
//             use std::io::Write;
//             let mut file = std::fs::File::create("d://code.bin").unwrap();
//             file.write_all(&self.code[..]).unwrap();
//             let mut file = std::fs::File::create("d://trampoline.bin").unwrap();
//             file.write_all(&self.trampoline.code[..]).unwrap();
//         }
//         let mut values = UnboxedValues::make_arguments(arguments, &self.signature);
//         let mut emulator = Unicorn::new(Arch::RISCV, Mode::RISCV64).unwrap();

//         let mut cur_memory_address = 0x1000 as u64;
//         let k_4: usize = 1024 * 4;

//         assert!(self.trampoline.data().len() < k_4);
//         assert!(self.code.len() < k_4);

//         let program_start = cur_memory_address;
//         emulator
//             .mem_map(cur_memory_address, k_4, Permission::all())
//             .unwrap();

//         emulator
//             .mem_write(cur_memory_address, self.trampoline.data())
//             .unwrap();
//         cur_memory_address += k_4 as u64;

//         // my code
//         let function_addr = cur_memory_address;
//         emulator
//             .mem_map(cur_memory_address, k_4, Permission::ALL)
//             .unwrap();
//         emulator
//             .mem_write(cur_memory_address, &self.code[..])
//             .unwrap();
//         cur_memory_address += k_4 as u64;
//         let program_end = program_start + self.trampoline.data().len() as u64 - 4;
//         // arguments
//         let mut arguments_data = values.make_vec_u8();
//         assert!(arguments_data.len() < k_4);
//         let argument_addr = cur_memory_address;
//         emulator
//             .mem_map(cur_memory_address, k_4, Permission::ALL)
//             .unwrap();
//         emulator
//             .mem_write(cur_memory_address, &arguments_data[..])
//             .unwrap();
//         cur_memory_address += k_4 as u64;

//         //paramter a0 and a1
//         emulator
//             .reg_write(RegisterRISCV::X10, function_addr)
//             .unwrap();
//         emulator
//             .reg_write(RegisterRISCV::X11, argument_addr)
//             .unwrap();
//         // sp
//         let sp_start = 2 * 1024 * 1024;
//         let extra_sp_size = 4 * 1024;
//         let sp_end = sp_start + 1024 * 1024; /*extra sp size*/
//         let reg_sp = RegisterRISCV::X2;
//         emulator.reg_write(reg_sp, sp_end).unwrap();
//         // map sp
//         emulator
//             .mem_map(
//                 sp_start,
//                 ((sp_end - sp_start) + extra_sp_size/* 假设有人在调用我 多余的1m是他的空间*/)
//                     as usize,
//                 Permission::all(),
//             )
//             .unwrap();
//         // emulator.mem_write(sp_start, &[0; 1024 * 1024][..]);
//         emulator
//             .add_code_hook(
//                 program_start,
//                 program_start + self.trampoline.data().len() as u64,
//                 |e, pc, _code_length| {
//                     let mut data = [0; 4];
//                     e.mem_read(pc, &mut data[..]).unwrap();
//                     println!("trampoline:{}", Self::dis(&data[..]));
//                 },
//             )
//             .unwrap();

//         emulator
//             .add_code_hook(
//                 function_addr,
//                 function_addr + self.code.len() as u64,
//                 |e, pc, _code_length| {
//                     if pc == function_addr {
//                         let print_arg = |name: &str, r: i32| {
//                             let value = e.reg_read(r).unwrap();
//                             println!("{}_u64:{},{}_i64:{}", name, value, name, value as i64)
//                         };
//                         for i in 0..=7 {
//                             print_arg(format!("a{}", i).as_str(), (RegisterRISCV::X10 as i32) + i);
//                         }
//                     }

//                     let mut data = [0; 4];
//                     e.mem_read(pc, &mut data[..]).unwrap();
//                     println!("test_function:{}", Self::dis(&data[..]));
//                 },
//             )
//             .unwrap();

//         let x = emulator.emu_start(
//             program_start,
//             program_start + self.trampoline.data().len() as u64 - 4,
//             0,
//             0,
//         );

//         match x {
//             Ok(_) => {}
//             Err(err) => {
//                 println!("err : {:?}", err);
//                 // print alot of status of emulator.
//                 {
//                     // pc
//                     let pc = emulator.pc_read().unwrap();
//                     let pc_in_code_range = || -> bool {
//                         pc >= function_addr && pc <= function_addr + self.code.len() as u64 - 4
//                     };
//                     let pc_trampline_range =
//                         || -> bool { pc >= program_start && pc <= program_end };
//                     if pc_in_code_range() {
//                         println!("pc in code range..")
//                     } else if pc_trampline_range() {
//                         println!("pc in trampline range..")
//                     } else {
//                         println!("pc({}) is out of range.", pc);
//                     }
//                 }

//                 let check_addr_in_sp_range = |addr: u64| {
//                     // sp
//                     println!(
//                         "addr {}  sp-sp_start={} sp_end-sp={}",
//                         addr,
//                         addr - sp_start,
//                         sp_end - addr
//                     );
//                     let sp_in_range = || addr >= sp_start && addr <= sp_end - 8;
//                     if sp_in_range() {
//                         println!("addr in range")
//                     } else if addr < sp_start {
//                         println!("addr is less than sp_start {}", sp_start - addr);
//                     } else {
//                         println!("addr is greater than sp_end {}", addr - sp_end);
//                     }
//                 };
//                 println!("check is sp in stack range");
//                 check_addr_in_sp_range(emulator.reg_read(reg_sp).unwrap());
//                 println!("check is sp in t2 range");
//                 check_addr_in_sp_range(emulator.reg_read(RegisterRISCV::X7).unwrap());
//                 x.unwrap()
//             }
//         }

//         // read back
//         emulator
//             .mem_read(argument_addr, &mut arguments_data[..])
//             .unwrap();

//         // write to origin vector
//         values.wirte_vec_u8_back(&arguments_data);
//         println!("result{:?}", values.0,);
//         emulator.emu_stop();
//         drop(emulator);
//         values.collect_returns(&self.signature)
//     }

//     pub fn x_reg_name(num: i32) -> String {
//         match num {
//             0 => "zero".into(),
//             1 => "ra".into(),
//             2 => "sp".into(),
//             3 => "gp".into(),
//             4 => "tp".into(),
//             5 => "t0".into(),
//             6..=7 => format!("t{}", num - 5),
//             8 => "fp".into(),
//             9 => "s1".into(),
//             10..=17 => format!("a{}", num - 10),
//             18..=27 => format!("s{}", num - 16),
//             28..=31 => format!("t{}", num - 25),
//             _ => unreachable!(),
//         }
//     }
//     pub fn f_reg_name(num: i32) -> String {
//         match num {
//             0..=7 => format!("ft{}", num - 0),
//             8..=9 => format!("fs{}", num - 8),
//             10..=17 => format!("fa{}", num - 10),
//             18..=27 => format!("fs{}", num - 16),
//             28..=31 => format!("ft{}", num - 20),
//             _ => unreachable!(),
//         }
//     }

//     fn dis(data: &[u8]) -> String {
//         let file_name = "d://one_instruction.bin";
//         use std::io::Write;
//         let mut file = std::fs::File::create(file_name).unwrap();
//         file.write_all(data).unwrap();
//         file.sync_all().unwrap();
//         use std::process::Command;
//         /* */
//         let mut cmd = Command::new("riscv64-unknown-elf-objdump");
//         let cmd = cmd.args(&["-b", "binary", "-m", "riscv:rv64", "-D", file_name]);
//         let output = cmd.output().expect("exec objdump failed , {}");
//         // println!(
//         //     "!!!!!!!!!!!!!!!!!!!{} {}",
//         //     String::from_utf8_lossy(&output.stderr[..]),
//         //     String::from_utf8_lossy(&output.stdout[..])
//         // );
//         let output = output.stdout;
//         /*
//             a.out:     file format elf64-littleriscv

//         Disassembly of section .text:

//         0000000000000000 <.text>:
//            0:   fe010113                addi    sp,sp,-32
//             */
//         let mut ret = String::default();
//         let mut i = 0;
//         while i < output.len() {
//             // match   0:
//             let mut _match = || -> bool {
//                 if output[i] == ('0' as u8) && output[i + 1] == (':' as u8) {
//                     i += 2;
//                     true
//                 } else {
//                     false
//                 }
//             };
//             if _match() {
//                 while i < output.len() {
//                     if output[i] == 10 {
//                         break;
//                     }
//                     ret.push(output[i] as char);
//                     i += 1;
//                 }
//             }
//             i += 1;
//         }
//         ret
//     }

//     fn print_emulator_statue<D>(emulator: &unicorn_engine::Unicorn<D>) {
//         for i in 1..=32 {
//             print!(
//                 "{:<5}{:<10}",
//                 format!("{}", Self::x_reg_name(i - 1)), /* start from zero */
//                 format!("({})", emulator.reg_read(i).unwrap())
//             );
//             if i == 32 {
//                 // print extra pc
//                 print!("pc:{}", emulator.reg_read(RegisterRISCV::PC).unwrap());
//             }
//             if i % 8 == 0 {
//                 println!()
//             }
//         }
//         println!("\n")
//     }

//     fn call2() -> Vec<DataValue> {
//         {
//             use std::io::Write;
//             let mut file = std::fs::File::create("d://code.bin").unwrap();
//             file.write_all(&self.code[..]).unwrap();
//             let mut file = std::fs::File::create("d://trampoline.bin").unwrap();
//             file.write_all(&self.trampoline.code[..]).unwrap();
//         }
//         use rvemu::emulator::Emulator;
//         let mut values = UnboxedValues::make_arguments(arguments, &self.signature);
//         let emulator = Emulator::new();

//         emulator.initialize_dram();
//         emulator.start();
//     }
// }
