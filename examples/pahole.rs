extern crate clap;
extern crate gimli;

use::rshole::{Parser, StructMemberIter};

fn main() {
    let path = "/home/jmill/kernel-junk/kernel-dbg/vmlinux".to_string();
    //let path = "/home/jmill/install/qemu/build/qemu-system-x86_64".to_string();

    println!("initializing dwarf parser...");
    let mut parser = Parser::new(path);

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
            if let Some(dw_type) = dw_struct_memb.mb_type {
                match dw_type {
                    rshole::Type::Struct(struct_type) => {
                        print!("  {} ", struct_type.name);
                        if let Some(mb_name) = dw_struct_memb.name {
                           println!("{}", mb_name);
                        }
                    }
                    rshole::Type::Base(base_type) => {
                        print!("  {} ", base_type.name);
                        if let Some(mb_name) = dw_struct_memb.name {
                           println!("{}", mb_name);
                        }
                    }
                    rshole::Type::Const(_const_type) => {
                        print!("  {} ", String::from("const"));
                        if let Some(mb_name) = dw_struct_memb.name {
                           println!("{}", mb_name);
                        }

                    }
                    _ => {}
                }
            }

        }

        println!("}}\n");
    }
}
