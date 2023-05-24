// #![deny(missing_docs)]
#![allow(dead_code)]

use std::{borrow::Cow, rc::Rc, fs::File};
use std::collections::HashMap;

use fallible_iterator::FallibleIterator;
use object::{Object, ObjectSection};
use memmap2::Mmap;
use gimli::{Reader, UnitOffset};

type R = gimli::EndianRcSlice<gimli::LittleEndian>;

#[derive(Clone, Debug)]
struct DwTypeMeta {
    offset: gimli::UnitOffset,
    header_idx: usize,
}

#[derive(Clone, Debug)]
pub struct Struct {
    pub name: String,
    pub size: u64,
    meta: DwTypeMeta,
    refcnt: u64
}

pub struct StructIter {
    parser: Parser,
    member_idx: usize
}

pub struct StructMember {
    pub name: Option<String>,
    pub size: u64,
    pub mb_type: Option<Type>,
    meta: DwTypeMeta
}

pub struct StructMemberIter<'a> {
    mb_struct: &'a Struct,
    parser: &'a Parser,
    member_idx: usize
}

pub struct StructUnionIter<'a> {
    dw_struct: &'a Struct,
    parser: &'a Parser,
    member_idx: usize
}

#[derive(Debug)]
pub struct AnonStruct {
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Typedef {
    pub name: String,
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Pointer {
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Subroutine {
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Array {
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Union {
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Const {
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Base {
    pub name: String,
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Enum {
    pub name: Option<String>,
    pub size: u64,
    meta: DwTypeMeta
}

#[derive(Debug)]
pub struct Unknown {
    meta: DwTypeMeta
}

#[derive(Debug)]
pub enum Type {
    Struct(Struct),
    Typedef(Typedef),
    Pointer(Pointer),
    Subroutine(Subroutine),
    Array(Array),
    Union(Union),
    Const(Const),
    Base(Base),
    Enum(Enum),
    Unknown(Unknown)
}

impl Type {
    fn get_meta(self) -> DwTypeMeta {
        let meta = match self {
            Type::Base(t) =>       { t.meta }
            Type::Array(t) =>      { t.meta }
            Type::Enum(t) =>       { t.meta }
            Type::Const(t) =>      { t.meta }
            Type::Typedef(t) =>    { t.meta }
            Type::Struct(t) =>     { t.meta }
            Type::Pointer(t) =>    { t.meta }
            Type::Union(t) =>      { t.meta }
            Type::Subroutine(t) => { t.meta }
            Type::Unknown(t) =>    { t.meta }
        };
        meta
    }
}

impl StructMember {
    fn new() -> StructMember {
        return StructMember {
            name: None,
            size: 0,
            mb_type: None,
            meta: DwTypeMeta { offset: gimli::UnitOffset(0), header_idx: 0 }
        }
    }
}

impl Iterator for StructMemberIter<'_> {
    type Item = StructMember;

    fn next(&mut self) -> Option<Self::Item> {
        //println!("next: {:?}", self.member_idx);
        let member = self.get_member(self.member_idx);
        let res = match member {
            Ok(memb) => {memb}
            Err(_) => {None}
        };
        self.member_idx += 1;
        return res
    }
}

impl StructMemberIter<'_> {
    pub fn new<'a>(mb_struct: &'a Struct, parser: &'a Parser) -> StructMemberIter<'a> {
        StructMemberIter { mb_struct, parser, member_idx: 0 }
    }

    pub fn get_member(&mut self, member_idx: usize) -> Result<Option<StructMember>, gimli::Error> {
        let mut iter = self.parser.sections.units().skip(self.mb_struct.meta.header_idx);
        while let Some(header) = iter.next()? {
            let unit = self.parser.sections.unit(header)?;
            let mut nested_entries = unit.entries_at_offset(self.mb_struct.meta.offset)?;

            // move iterator to member index
            for _ in 0..=member_idx { nested_entries.next_dfs()?; }

            // return next member or None
            while let Some((_delta_depth, entry)) = nested_entries.next_dfs()? {
                if entry.tag() != gimli::DW_TAG_member {
                    return Ok(None)
                }
                return self.parse_member(entry);
            }
        }
        Ok(None)
    }

    fn parse_member(&mut self, entry: &gimli::DebuggingInformationEntry<R>) -> Result<Option<StructMember>, gimli::Error> {
        let mut attrs = entry.attrs();
        let mut member = StructMember::new();
        // println!("    main tag: {}", entry.tag());
        while let Some(attr) = attrs.next()? {
            match attr.name() {
                gimli::DW_AT_type => {
                    match attr.value() {
                        gimli::AttributeValue::UnitRef(offset) => {
                            member.mb_type = Some(self.parser.get_type_meta(self.mb_struct.meta.header_idx, offset)?);
                        },
                        _ => ()
                    }
                }
                gimli::DW_AT_name => {
                    member.name = name_attr_to_string(&self.parser.sections.debug_str, &attr)?;
                }
                gimli::DW_AT_byte_size => {
                    let member_size = attr.value().udata_value();
                    member.size = member_size.unwrap_or(0);
                }
                _ => {}
            }
        }
        Ok(Some(member))
    }

}

pub struct Parser {
    sections: gimli::Dwarf<R>,
    pub struct_dict: HashMap<String, Struct>
}

impl Parser {
    pub fn new(file: File) -> Parser {
        let sections = Self::load_sections(file);
        let struct_dict = HashMap::<String, Struct>::new();
        Parser { sections, struct_dict }
    }

    fn load_sections(file: File) -> gimli::Dwarf<R> {
        // src: https://github.com/tchajed/rdb/blob/main/src/dwarf.rs#L252
        let map = unsafe { Mmap::map(&file).unwrap() };

        let object = object::File::parse(&*map).unwrap();

        let load_section = |id: gimli::SectionId| -> Result<R, gimli::Error> {
            let data = object
                .section_by_name(id.name())
                .and_then(|section| section.uncompressed_data().ok())
                .unwrap_or(Cow::Borrowed(&[][..]));
            Ok(R::new(Rc::from(&*data), gimli::LittleEndian))
        };

        let dwarf = gimli::Dwarf::load(&load_section).expect("Failed to load dwarf sections");

        return dwarf;
    }

    pub fn load_structs(&mut self) -> Result<(), gimli::Error> {
        let mut iter = self.sections.units();
        let mut header_idx = 0;
        while let Some(header) = iter.next()? {
            let unit = self.sections.unit(header)?;
            let mut entries = unit.entries();
            while let Some((_delta_depth, entry)) = entries.next_dfs()? {
                if entry.tag() != gimli::DW_TAG_structure_type {
                    continue;
                }
                self.load_struct(header_idx, entry)?;
            }
            header_idx += 1;
        }
        Ok(())
    }

    pub fn load_struct(&mut self, header_idx: usize, entry: &gimli::DebuggingInformationEntry<R>) -> Result<(), gimli::Error> {
        let mut attrs = entry.attrs();
        let mut struct_name: Option<String> = None;
        let mut struct_size: Option<u64> = None;
        while let Some(attr) = attrs.next()? {
            match attr.name() {
                gimli::DW_AT_name => {
                    struct_name = name_attr_to_string(&self.sections.debug_str, &attr)?;
                }
                gimli::DW_AT_byte_size => {
                    struct_size = attr.value().udata_value();
                }
                gimli::DW_AT_declaration => {
                    // just say empty set to declarations
                    return Ok(());
                }
                _ => {}
            }
            if struct_name.is_some() && struct_size.is_some() {
                break;
            }
        }
        if let Some(name) = &struct_name {
            let size = struct_size.unwrap_or(0);
            match self.struct_dict.entry(name.clone()) {
                std::collections::hash_map::Entry::Occupied(mut dentry) => {
                    dentry.get_mut().refcnt += 1;
                }
                std::collections::hash_map::Entry::Vacant(dentry) => {
                    let meta = DwTypeMeta { offset: entry.offset(), header_idx };
                    dentry.insert(Struct{name: name.clone(), size, meta, refcnt: 0});
                }
            };
        }
        Ok(())
    }

    pub fn get_type(&self, type_inst: Type ) -> Result<Option<Type>, gimli::Error> {
        //println!("get_type({:?})", type_inst);
        let meta = type_inst.get_meta();
        let mut iter = self.sections.units().skip(meta.header_idx);
        while let Some(header) = iter.next()? {
            let unit = self.sections.unit(header)?;
            let mut nested_entries = unit.entries_at_offset(meta.offset)?;

            if let Some((_delta_depth, entry)) = nested_entries.next_dfs()? {
                let mut attrs = entry.attrs();
                while let Some(attr) = attrs.next()? {
                    match attr.name() {
                        gimli::DW_AT_type => {
                            match attr.value() {
                                gimli::AttributeValue::UnitRef(offset) => {
                                    let _type = self.get_type_meta(meta.header_idx, offset)?;
                                    return Ok(Some(_type));
                                },
                                _ => ()
                            }
                        }
                        _ => {}
                    }
                }
                return Ok(None);
            }
        }
        // FIXME
        Err(gimli::Error::TypeMismatch)
    }

    fn get_array_bounds(&self, header_idx: usize, arr_offset: UnitOffset) -> Result<u64, gimli::Error> {
        let mut iter = self.sections.units().skip(header_idx);

        if let Some(header) = iter.next()? {
            let unit = self.sections.unit(header)?;
            let mut nested_entries = unit.entries_at_offset(arr_offset)?;
            let _ = nested_entries.next_dfs(); // skip one
            if let Some(dfs) = nested_entries.next_dfs()? {
                let type_dfs = dfs.1;
                let tag = type_dfs.tag();

                // println!("    type tag: {}", type_dfs.tag());

                let mut attrs = type_dfs.attrs();
                match tag {
                    gimli::DW_TAG_subrange_type => {
                        while let Some(attr) = attrs.next()? {
                            match attr.name() {
                                gimli::DW_AT_upper_bound => {
                                    return Ok(attr.value().udata_value().unwrap_or(0) + 1)
                                }
                                _ => {}
                            }
                           // println!("    type attr: {}", attr.name());
                        }
                    }
                    _ => {
                        return Err(gimli::Error::TypeMismatch) // FIXME
                    }
                }
            }
        }
        Ok(0)
    }

    fn get_type_meta(&self, header_idx: usize, offset: UnitOffset) -> Result<Type, gimli::Error> {
        let mut iter = self.sections.units().skip(header_idx);
        let meta = DwTypeMeta { offset, header_idx };

        if let Some(header) = iter.next()? {
            let unit = self.sections.unit(header)?;
            let mut nested_entries = unit.entries_at_offset(offset)?;
            if let Some(dfs) = nested_entries.next_dfs()? {
                let type_dfs = dfs.1;
                let tag = type_dfs.tag();

                // println!("    type tag: {}", type_dfs.tag());

                let mut attrs = type_dfs.attrs();
                match tag {
                    gimli::DW_TAG_structure_type => {
                        let mut size: u64 = 0;
                        while let Some(attr) = attrs.next()? {
                            // println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_name => {
                                    if let Some(name) = name_attr_to_string(&self.sections.debug_str, &attr)? {
                                        let entry = self.struct_dict.get(&name);
                                        match entry {
                                            Some(entry) => {
                                                return Ok(Type::Struct( Struct {
                                                    name: entry.name.to_string(),
                                                    meta, size,
                                                    refcnt: 0,
                                                }));
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                gimli::DW_AT_byte_size => {
                                    size = attr.value().udata_value().unwrap_or(0);
                                }
                                _ => { }
                            }
                        }
                        // handle anon struct
                        return Ok(Type::Struct(
                                    Struct {
                                        name: String::from("void"),
                                        meta, size, refcnt: 1
                                    }
                                ));
                    }
                    gimli::DW_TAG_typedef => {
                        let mut name: String = String::new();
                        let mut size: u64 = 0;
                        while let Some(attr) = attrs.next()? {
                            // println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_name => {
                                    name = name_attr_to_string(&self.sections.debug_str, &attr)?.unwrap_or(String::from("wtf"));
                                }
                                gimli::DW_AT_byte_size => {
                                    size = attr.value().udata_value().unwrap_or(0);
                                }
                                _ => { }
                            }
                        }
                        return Ok(Type::Typedef( Typedef { name, meta, size }));
                    }
                    gimli::DW_TAG_pointer_type => {
                        //while let Some(attr) = attrs.next()? {
                        //    println!("    type attr: {}", attr.name());
                        //}
                        return Ok(Type::Pointer( Pointer{ meta, size: 8 } ));
                    }
                    gimli::DW_TAG_const_type => {
                        // while let Some(attr) = attrs.next()? {
                        //     println!("    type attr: {}", attr.name());
                        // }
                        return Ok(Type::Const( Const{ meta, size: 8 } ));
                    }
                    gimli::DW_TAG_base_type => {
                        let mut name: String = String::new();
                        let size: u64 = 0;
                        while let Some(attr) = attrs.next()? {
                            // println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_name => {
                                    name = name_attr_to_string(&self.sections.debug_str, &attr)?.unwrap_or(String::from("void"));
                                }
                                _ => { }
                            }
                        }
                        return Ok(Type::Base( Base{ name, size, meta } ))
                    }
                    gimli::DW_TAG_union_type => {
                        // mb_type.type_tag = MemberType::Union;
                        let size = 0;
                        // while let Some(attr) = attrs.next()? {
                        //    println!("    type attr: {}", attr.name());
                        // }
                        return Ok(Type::Union( Union{ size, meta } ))
                    }
                    gimli::DW_TAG_array_type => {
                        // Array types are immediately followed by a DW_TAG_subrange_type
                        // which describes the array size in the upper_bound
                        //while let Some(attr) = attrs.next()? {
                        //   println!("    type attr: {}", attr.name());
                        //}
                        let bounds = self.get_array_bounds(header_idx, offset)?;
                        // println!("bounds: {}", bounds);
                        return Ok(Type::Array( Array{ size: bounds, meta } ))
                    }
                    gimli::DW_TAG_enumeration_type => {
                        // mb_type.type_tag = MemberType::Enum;
                        let size = 0;
                        let mut name = None;
                        while let Some(attr) = attrs.next()? {
                            // println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_name => {
                                    name = name_attr_to_string(&self.sections.debug_str, &attr)?;
                                }
                                _ => { }
                            }
                        }
                        return Ok(Type::Enum( Enum{ name, size, meta } ));
                    }
                    gimli::DW_TAG_subroutine_type => {
                        // mb_type.type_tag = MemberType::Subroutine;
                        let size = 0;
                        //while let Some(attr) = attrs.next()? {
                        //    println!("    type attr: {}", attr.name());
                        //}
                        return Ok(Type::Subroutine( Subroutine{ size, meta } ));
                    }
                    gimli::DW_TAG_formal_parameter => {
                        let size = 0;
                        //while let Some(attr) = attrs.next()? {
                        //    println!("    type attr: {}", attr.name());
                        //}
                        return Ok(Type::Subroutine( Subroutine{ size, meta } ));
                    }
                    _ => {
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                        }
                    }
                }
            }
        }
        // FIXME
        Err(gimli::Error::TypeMismatch)
    }

}

fn name_attr_to_string(debug_str: &gimli::DebugStr<R>, attr: &gimli::Attribute<gimli::EndianReader<gimli::LittleEndian, Rc<[u8]>>>) -> Result<Option<String>, gimli::Error> {
    let name = match attr.value() {
        gimli::AttributeValue::String(val) => {
            Some(val.to_string_lossy()?.to_string())
        }
        gimli::AttributeValue::DebugStrRef(val) => {
            Some(debug_str.get_str(val)?.to_string_lossy()?.to_string())
        }
        _ => {None}
    };
    Ok(name)
}
