extern crate clap;
extern crate gimli;

use std::fs::File;
use std::env;
use::rshole::{Parser, StructMemberIter};

// recursive string builder
fn get_member_string(parser: &rshole::Parser, mb_type: rshole::Type) -> Result<String, gimli::Error> {
    match mb_type {
        rshole::Type::Struct(struct_type) => {
            let struct_name = struct_type.name;
            return Ok(format!("{} ", struct_name));
        }
        rshole::Type::Base(base_type) => {
            let base_name = base_type.name;
            return Ok(format!("{} ", base_name));
        }
        rshole::Type::Typedef(typedef_type) => {
            let typedef_name = typedef_type.name;
            return Ok(format!("{} ", typedef_name));
        }
        rshole::Type::Const(_) => {
            if let Some(inner_type) = parser.get_type(mb_type)? {
                let inner_string = get_member_string(&parser, inner_type)?;
                return Ok(format!("const {}", inner_string));
            }
        }
        rshole::Type::Pointer(_) => {
            if let Some(inner_type) = parser.get_type(mb_type)? {
                //println!("--{:?}--", inner_type);
                let inner_string = get_member_string(&parser, inner_type)?;
                //let inner_string = String::new();
                return Ok(format!("{}*", inner_string));
            }
            return Ok(String::from("void *"));
        }
        rshole::Type::Enum(ref enum_type) => {
            if let Some(enum_name) = &enum_type.name {
                return Ok(format!("enum {} ", enum_name));
            }
            // TODO: if not named need to iterate to get enumerators
            // 0x00000034:     DW_TAG_enumeration_type
            //         ...
            // 0x0000003b:       DW_TAG_enumerator
            //         DW_AT_name	("test")
            //         DW_AT_const_value	(8)
            // 0x00000041:       DW_TAG_enumerator
            //         DW_AT_name	("test2")
            //         DW_AT_const_value	(4)

            if let Some(inner_type) = parser.get_type(mb_type)? {
                let inner_string = get_member_string(&parser, inner_type)?;
                return Ok(format!("enum {}", inner_string));
            }
            // should never be reached
            return Ok(String::from("enum anon "))
        }
        _ => {}
    }
    return Ok(String::new());
}

fn main() -> Result<(), gimli::Error> {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];
    //let path = "/home/jmill/kernel-junk/kernel-dbg/vmlinux".to_string();
    //let path = "/home/jmill/install/qemu/build/qemu-system-x86_64".to_string();

    let file = File::open(&path)?;

    println!("initializing dwarf parser...");
    let mut parser = Parser::new(file);

    println!("loading structs from dwarf info...");
    parser.load_structs().expect("Failed to load structs");


    println!("found structs:");
    for (_name, dw_struct) in parser.struct_dict.iter() {
        // if dw_struct.name != "cryptomgr_param" {
        //     continue;
        // }
        let mut iter = StructMemberIter::new(&dw_struct, &parser);

        println!("struct {} {{", dw_struct.name);

        while let Some(dw_struct_memb) = iter.next() {
            if let Some(mb_type) = dw_struct_memb.mb_type {
                let member_string = get_member_string(&parser, mb_type)?;
                if let Some(mb_name) = dw_struct_memb.name {
                    println!("  {}{}", member_string, mb_name);
                }
            }
        }

        println!("}}\n");
    }
    Ok(())
}
