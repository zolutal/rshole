extern crate clap;
extern crate gimli;

use rshole::MemberType;
use::rshole::{Parser, DwStructMemberIter};

fn main() {
    let path = "/home/jmill/kernel-junk/kernel-dbg/vmlinux".to_string();
    //let path = "/home/jmill/install/qemu/build/qemu-system-x86_64".to_string();

    println!("initializing dwarf parser...");
    let mut parser = Parser::new(path);

    println!("loading structs from dwarf info...");
    parser.load_structs().expect("Failed to load structs");

    println!("found structs:");
    for dw_struct in parser.struct_dict.values() {
        // if dw_struct.name != "cryptomgr_param" {
        //     continue;
        // }
        let mut iter = DwStructMemberIter::new(&dw_struct, &parser);
        println!("struct {} {{", dw_struct.name);
        while let Some(dw_struct_memb) = iter.next() {
            if let Some(dw_type) = dw_struct_memb.mb_type {
                print!("  {} ", dw_type.name.unwrap_or("?".to_string()));
                if let MemberType::Pointer = dw_type.type_tag {
                    print!(" *");
                    if let Some(name) = dw_struct_memb.name {
                        println!("{}", name);
                    }
                } else if let MemberType::Subroutine = dw_type.type_tag {
                    if let Some(name) = dw_struct_memb.name {
                        println!("(*{})", name);
                    }
                } else {
                    if let Some(name) = dw_struct_memb.name {
                        println!("{}", name);
                    }
                }
            }

        }
        println!("}}\n");
    }
}
