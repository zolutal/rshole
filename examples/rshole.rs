extern crate clap;
extern crate gimli;

use::rshole::StructMemberIter;
use std::fs::File;
use clap::Parser;

// recursive string builder
fn get_member_string(parser: &rshole::Parser, mb_type: rshole::Type, mb_name: &String, level: u8) -> Result<String, gimli::Error> {
    match mb_type {
        rshole::Type::Struct(struct_type) => {
            let struct_name = struct_type.name;
            if level == 0 {
                return Ok(format!("struct {} {}", struct_name, mb_name));
            }
            return Ok(format!("struct {} ", struct_name));
        }
        rshole::Type::Base(base_type) => {
            let base_name = base_type.name;
            if level == 0 {
                return Ok(format!("{} {}", base_name, mb_name));
            }
            return Ok(format!("{} ", base_name));
        }
        rshole::Type::Typedef(typedef_type) => {
            let typedef_name = typedef_type.name;
            if level == 0 {
                return Ok(format!("{} {}", typedef_name, mb_name));
            }
            return Ok(format!("{} ", typedef_name));
        }
        rshole::Type::Const(_) => {
            if let Some(inner_type) = parser.get_type(mb_type)? {
                let inner_string = get_member_string(&parser, inner_type, mb_name, level+1)?;
                return Ok(format!("const {}", inner_string));
            }
        }
        rshole::Type::Pointer(_) => {
            if let Some(inner_type) = parser.get_type(mb_type)? {
                let inner_string = get_member_string(&parser, inner_type, mb_name, level+1)?;
                if level == 0 {
                    return Ok(format!("{}*{}", inner_string, mb_name));
                }
                return Ok(format!("{}*", inner_string));
            }
            return Ok(format!("void * {}", mb_name));
        }
        rshole::Type::Enum(ref enum_type) => {
            if let Some(enum_name) = &enum_type.name {
                return Ok(format!("enum {}", enum_name));
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
                let inner_string = get_member_string(&parser, inner_type, mb_name, level+1)?;
                return Ok(format!("enum {} {}", inner_string, mb_name));
            }
            // should never be reached
            return Ok(String::from("enum anon "))
        }
        rshole::Type::Array(ref arr_type) => {
            let arr_size = arr_type.size;

            if let Some(inner_type) = parser.get_type(mb_type)? {
                let inner_string = get_member_string(&parser, inner_type, mb_name, level+1)?;
                if arr_size == 0 {
                    return Ok(format!("{}{}[]", inner_string, mb_name));
                }
                return Ok(format!("{}{}[{}]", inner_string, mb_name, arr_size));
            }
            return Ok(format!("{}[?]", mb_name));
        }
        rshole::Type::Subroutine(_) => {
            if level == 0 {
                return Ok(format!("subroutine {}", mb_name));
            }
                return Ok(format!("subroutine "));
        }
        rshole::Type::Union(_) => {
            if level == 0 {
                return Ok(format!("union {}", mb_name));
            }
                return Ok(format!("union "));
        }
        _ => {
            //println!("Unhandled: {:?}", mb_type)
        }
    }
    return Ok(String::new());
}

fn print_struct(dw_struct: &rshole::Struct, parser: &rshole::Parser) -> Result<(), gimli::Error> {
    let mut iter = StructMemberIter::new(&dw_struct, &parser);

    println!("struct {} {{", dw_struct.name);
    while let Some(dw_struct_memb) = iter.next() {
        if let Some(mb_type) = dw_struct_memb.mb_type {
            if let Some(mb_name) = dw_struct_memb.name {
                let member_string = get_member_string(&parser, mb_type, &mb_name, 0)?;
                println!("  {};", member_string);
            }
        }
    }
    println!("}}\n");

    Ok(())
}

#[derive(clap::Parser, Debug)]
struct Args {
    path: String,
    name: Option<String>
}


fn main() -> Result<(), gimli::Error> {
    let args = Args::parse();
    let file = File::open(args.path)?;

    println!("initializing dwarf parser...");
    let mut parser = rshole::Parser::new(file);

    println!("loading structs from dwarf info...");
    parser.load_structs().expect("Failed to load structs");

    match args.name {
        Some(arg_name) => {
            println!("found struct:");
            for (struct_name, dw_struct) in parser.struct_dict.iter() {
                if arg_name.eq(struct_name) {
                    print_struct(dw_struct, &parser)?;
                }
            }
        }
        _ => {
            println!("found structs:");
            for (_name, dw_struct) in parser.struct_dict.iter() {
                print_struct(dw_struct, &parser)?;
            }
        }
    }

    Ok(())
}
