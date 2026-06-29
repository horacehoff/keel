use crate::RED;
use crate::RESET;
use crate::compiler_data::{Function, Pools};
use crate::errors::GREEN;
use crate::errors::YELLOW;
use crate::type_system::DataType;
use crate::{data::Data, instr::Instr};
use smol_strc::{SmolStr, ToSmolStr};
use std::hint::unreachable_unchecked;

pub fn format_data(
    x: Data,
    array_pool: &[Vec<Data>],
    string_pool: &[String],
    struct_fields: &[(SmolStr, Vec<SmolStr>)],
    show_str: bool,
) -> SmolStr {
    if x.is_float() {
        x.as_float().to_smolstr()
    } else if x.is_int() {
        x.as_int().to_smolstr()
    } else if x.is_bool() {
        x.as_bool().to_smolstr()
    } else if x.is_str() {
        if show_str {
            x.as_str(string_pool).to_smolstr()
        } else {
            format_args!("\"{}\"", x.as_str(string_pool)).to_smolstr()
        }
    } else if x.is_array() {
        format_args!("[{}]", unsafe {
            array_pool
                .get_unchecked(x.as_array())
                .iter()
                .map(|x| format_data(*x, array_pool, string_pool, struct_fields, false))
                .collect::<Vec<SmolStr>>()
                .join(",")
        })
        .to_smolstr()
    } else if x.is_null() {
        SmolStr::new_static("null")
    } else if x.is_struct() {
        let s_name = unsafe { &struct_fields.get_unchecked(x.struct_type_id() as usize).0 };
        format_args!("{} {{{}}}", s_name, unsafe {
            array_pool
                .get_unchecked(x.as_struct())
                .iter()
                .map(|x| {
                    format_args!(
                        "{}",
                        // s_fields.get_unchecked(i),
                        format_data(*x, array_pool, string_pool, struct_fields, false)
                    )
                    .to_smolstr()
                })
                .collect::<Vec<SmolStr>>()
                .join(",")
        })
        .to_smolstr()
    } else {
        unsafe { unreachable_unchecked() }
    }
}

pub fn get_type_name(x: &Data) -> &str {
    if x.is_array() {
        "Array"
    } else if x.is_bool() {
        "Boolean"
    } else if x.is_str() {
        "String"
    } else if x.is_float() {
        "Float"
    } else if x.is_int() {
        "Integer"
    } else if x.is_null() {
        "Null"
    } else if x.is_struct() {
        "Struct"
    } else {
        unreachable!()
    }
}

pub fn print_debug(
    instructions: &[Instr],
    registers: &[Data],
    pools: &Pools,
    struct_fields: &[(SmolStr, Vec<SmolStr>)],
) {
    println!("{YELLOW}---- DEBUG ----{RESET}");
    if !pools.obj_pool.is_empty() {
        println!("{GREEN}---  ARRAYS  ---{RESET}");
        for (i, data) in pools.obj_pool.iter().enumerate() {
            println!(" {i} {data:?}");
        }
    }
    println!("{GREEN}-- REGISTERS --{RESET}");
    for (i, data) in registers.iter().enumerate() {
        println!(
            " [{i}] {}({})",
            get_type_name(data),
            format_data(
                *data,
                &pools.obj_pool,
                &pools.string_pool,
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
