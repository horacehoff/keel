use crate::parser_data::{Function, Pools};
use crate::type_system::DataType;
use crate::{data::Data, instr::Instr};
use inline_colorization::*;
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
        SmolStr::new_static("NULL")
    } else if x.is_struct() {
        let (s_name, s_fields) =
            unsafe { struct_fields.get_unchecked(x.struct_type_id() as usize) };
        format_args!("{} {{{}}}", s_name, unsafe {
            array_pool
                .get_unchecked(x.as_struct())
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    format_args!(
                        "{}:{}",
                        s_fields.get_unchecked(i),
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

pub fn _display_fn_signatures(f: Function) {
    for fn_impl in f.impls {
        let return_type = f
            .return_type_cache
            .iter()
            .find(|(args, _)| *args == fn_impl.arg_types)
            .map_or(DataType::Null, |(_, ret)| ret.clone());
        println!(
            "{} : ({}) -> {}",
            f.name,
            fn_impl
                .arg_types
                .iter()
                .map(|x| x.to_smolstr())
                .collect::<Vec<_>>()
                .join(", "),
            {
                if return_type == DataType::Null {
                    SmolStr::new_static("()")
                } else {
                    return_type.to_smolstr()
                }
            }
        );
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
    println!("{color_yellow}---- DEBUG ----{color_reset}");
    if !pools.obj_pool.is_empty() {
        println!("{color_green}---  ARRAYS  ---{color_reset}");
        for (i, data) in pools.obj_pool.iter().enumerate() {
            println!(" {i} {data:?}");
        }
    }
    println!("{color_green}-- REGISTERS --{color_reset}");
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
    println!("{color_red}-- INSTRUCTIONS --{color_reset}");
    for (i, instr) in instructions.iter().enumerate() {
        println!(" {i}: {instr:?}");
    }
    println!("{color_yellow}------------------{color_reset}");
}
