use crate::RED;
use crate::RESET;
use crate::compiler_data::Pools;
use crate::errors::GREEN;
use crate::errors::YELLOW;
use crate::{data::Data, instr::Instr};
use smol_strc::SmolStr;

pub fn print_debug(
    instructions: &[Instr],
    registers: &[Data],
    pools: &Pools,
    struct_fields: &[(SmolStr, Vec<SmolStr>)],
) {
    println!("{YELLOW}---- DEBUG ----{RESET}");
    if !pools.objs.is_empty() {
        println!("{GREEN}---  ARRAYS  ---{RESET}");
        for (i, data) in pools.objs.iter().enumerate() {
            println!(" {i} {data:?}");
        }
    }
    println!("{GREEN}-- REGISTERS --{RESET}");
    for (i, data) in registers.iter().enumerate() {
        println!(
            " [{i}] {}({})",
            data.type_name(),
            data.format(
                &pools.objs,
                &pools.strings,
                &pools.maps,
                struct_fields,
                true
            )
        );
    }
    if instructions.is_empty() {
        return;
    }
    println!("{RED}-- INSTRUCTIONS --{RESET}");
    for (i, instr) in instructions.iter().enumerate() {
        println!(" {i}: {instr:?}");
    }
    println!("{YELLOW}------------------{RESET}");
}
