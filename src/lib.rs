// #![deny(missing_docs)]
#![allow(dead_code)]

use std::{borrow::Cow, rc::Rc, fs::File};
use std::collections::HashMap;

use fallible_iterator::FallibleIterator;
use object::{Object, ObjectSection};
use memmap2::Mmap;
use gimli::{Reader, UnitOffset};

type R = gimli::EndianRcSlice<gimli::LittleEndian>;


/* API brainstorming
    class RsHoleTypeStruct
    class RsHoleTypeUnion
    class RsHoleTypeArray

   for dw_struct in rshole::Parser(fp).iter_structs():
        print(dw_struct.name)
        for dw_member in dw_struct:
            print(dw_member.type_name, end='')

            if (dw_member.is_ptr)
                print(' *', end='')

            print(dw_member.name, end='')

            if (dw_member.is_array):
                print(f'[{dw_member.arr_range}]', end='')

            print(';')
*/


pub struct DwStruct {
    pub name: String,
    pub size: u64,
    offset: gimli::UnitOffset,
    refcnt: u64,
    header_idx: usize
}

pub struct DwStructIter {
    parser: Parser,
    member_idx: usize
}

pub struct DwStructMember {
    pub name: Option<String>,
    pub size: u64,
    pub mb_type: Option<Type>,
    offset: gimli::UnitOffset,
    header_idx: usize
}

pub struct DwStructMemberIter<'a> {
    dw_struct: &'a DwStruct,
    parser: &'a Parser,
    member_idx: usize
}

pub struct DwStructUnionIter<'a> {
    dw_struct: &'a DwStruct,
    parser: &'a Parser,
    member_idx: usize
}

pub enum MemberType {
    AnonStruct,
    Struct,
    Typedef,
    Pointer,
    Subroutine,
    Array,
    Union,
    Const,
    Base,
    Enum,
    Unknown,
}

pub struct Type {
    pub name: Option<String>,
    pub size: u64,
    pub type_tag: MemberType,

    members: Option<Vec<String>>, // struct members
    range: Option<u64>,           // # of array elements

    tag: gimli::DwTag,
    offset: gimli::UnitOffset,
    header_idx: usize,
}

impl Type {
    fn new() -> Type {
        Type {
            name: None,
            size: 0,
            type_tag: MemberType::Unknown,

            members: None,
            range: None,

            tag: gimli::DW_TAG_null,
            offset: gimli::UnitOffset(0),
            header_idx: 0
        }
    }

    fn deref() -> Result<(), gimli::Error> {
        // get the type associated with a pointer or array
        Ok(())
    }

    fn get_member(&mut self, member_idx: usize) -> Result<(), gimli::Error> {
        // for a Union get the member at the specified index
        Ok(())
    }
}

impl DwStructMember {
    fn new() -> DwStructMember {
        return DwStructMember {
            name: None,
            size: 0,
            mb_type: None,
            offset:
            gimli::UnitOffset(0),
            header_idx: 0
        }
    }
}

impl Iterator for DwStructMemberIter<'_> {
    type Item = DwStructMember;

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

impl DwStructMemberIter<'_> {
    pub fn new<'a>(dw_struct: &'a DwStruct, parser: &'a Parser) -> DwStructMemberIter<'a> {
        DwStructMemberIter { dw_struct, parser, member_idx: 0 }
    }

    pub fn get_member(&mut self, member_idx: usize) -> Result<Option<DwStructMember>, gimli::Error> {
        let mut iter = self.parser.sections.units().skip(self.dw_struct.header_idx);
        while let Some(header) = iter.next()? {
            let unit = self.parser.sections.unit(header.clone())?;
            let mut nested_entries = unit.entries_at_offset(self.dw_struct.offset)?;

            // iterate to member index
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

    fn parse_member(&mut self, entry: &gimli::DebuggingInformationEntry<R>) -> Result<Option<DwStructMember>, gimli::Error> {
        let mut attrs = entry.attrs();
        let mut member = DwStructMember::new();
        println!("    main tag: {}", entry.tag());
        while let Some(attr) = attrs.next()? {
            match attr.name() {
                gimli::DW_AT_type => {
                    match attr.value() {
                        gimli::AttributeValue::UnitRef(offset) => {
                            member.mb_type = Some(self.get_type(offset)?);
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

    // how to handle protoyped well?
    // need to consider:
    // return type, is return type pointer?
    // member type and name
    // arguments? how??
    fn get_type(&mut self, offset: UnitOffset) -> Result<Type, gimli::Error> {
        let mut iter = self.parser.sections.units().skip(self.dw_struct.header_idx);
        let mut mb_type: Type = Type::new();
        mb_type.header_idx = self.dw_struct.header_idx;
        mb_type.offset = offset;

        if let Some(header) = iter.next()? {
            let unit = self.parser.sections.unit(header.clone())?;
            let mut nested_entries = unit.entries_at_offset(offset)?;
            if let Some(dfs) = nested_entries.next_dfs()? {
                let type_dfs = dfs.1;
                mb_type.tag = type_dfs.tag();

                println!("    type tag: {}", type_dfs.tag());

                let mut attrs = type_dfs.attrs();
                match mb_type.tag {
                    gimli::DW_TAG_structure_type => {
                        mb_type.type_tag = MemberType::Struct;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_name => {
                                    mb_type.name = name_attr_to_string(&self.parser.sections.debug_str, &attr)?;
                                }
                                _ => { break }
                            }
                        }
                        // nameless struct is anon
                        if mb_type.name.is_none() {
                            mb_type.type_tag = MemberType::Struct;
                        }
                    }
                    gimli::DW_TAG_typedef => {
                        mb_type.type_tag = MemberType::Typedef;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_name => {
                                    mb_type.name = name_attr_to_string(&self.parser.sections.debug_str, &attr)?;
                                }
                                _ => { break }
                            }
                        }
                    }
                    gimli::DW_TAG_pointer_type => {
                        mb_type.type_tag = MemberType::Pointer;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_type => {
                                    match attr.value() {
                                        gimli::AttributeValue::UnitRef(offset) => {
                                            let nested = self.get_type(offset)?;
                                            mb_type.name = nested.name;
                                            if let MemberType::Subroutine = nested.type_tag {
                                                mb_type.type_tag = MemberType::Subroutine;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                _ => { }
                            }
                        }
                        if mb_type.name.is_none() {
                            mb_type.name = Some("void".to_string());
                        }
                    }
                    gimli::DW_TAG_const_type => {
                        mb_type.type_tag = MemberType::Const;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_type => {
                                    match attr.value() {
                                        gimli::AttributeValue::UnitRef(offset) => {
                                            let nested = self.get_type(offset)?;
                                            mb_type.name = nested.name;
                                        }
                                        _ => {}
                                    }
                                }
                                _ => { }
                            }
                        }
                    }
                    gimli::DW_TAG_base_type => {
                        mb_type.type_tag = MemberType::Base;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_name => {
                                    mb_type.name = name_attr_to_string(&self.parser.sections.debug_str, &attr)?;
                                }
                                _ => { }
                            }
                        }
                    }
                    gimli::DW_TAG_union_type => {
                        mb_type.type_tag = MemberType::Union;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                        }
                    }
                    gimli::DW_TAG_enumeration_type => {
                        mb_type.type_tag = MemberType::Enum;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                        }
                    }
                    gimli::DW_TAG_subroutine_type => {
                        mb_type.type_tag = MemberType::Subroutine;
                        while let Some(attr) = attrs.next()? {
                            println!("    type attr: {}", attr.name());
                            match attr.name() {
                                gimli::DW_AT_type => {
                                    match attr.value() {
                                        gimli::AttributeValue::UnitRef(offset) => {
                                            let nested = self.get_type(offset)?;
                                            mb_type.name = nested.name;
                                        }
                                        _ => {}
                                    }
                                }
                                _ => { }
                            }
                            if mb_type.name.is_none() {
                                mb_type.name = Some("void".to_string());
                            }
                        }
                    }
                    _ => { }
                }
            }
        }
        Ok(mb_type)
    }
}

pub struct Parser {
    sections: gimli::Dwarf<R>,
    pub struct_dict: HashMap<String, DwStruct>
}

impl Parser {
    pub fn new(path: String) -> Parser {
        let sections = Self::load_sections(path);
        let struct_dict = HashMap::<String, DwStruct>::new();
        Parser { sections, struct_dict }
    }

    fn load_sections(path: String) -> gimli::Dwarf<R> {
        // src: https://github.com/tchajed/rdb/blob/main/src/dwarf.rs#L252
        let file = File::open(&path).unwrap();
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
            let unit = self.sections.unit(header.clone())?;
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
        if struct_name.is_some() {
            let name = struct_name.unwrap();
            let size = struct_size.unwrap_or(0);
            match self.struct_dict.entry(name.clone()) {
                std::collections::hash_map::Entry::Occupied(mut dentry) => {
                    dentry.get_mut().refcnt += 1;
                }
                std::collections::hash_map::Entry::Vacant(dentry) => {
                    dentry.insert(DwStruct{name, size, offset: entry.offset(), header_idx, refcnt: 0});
                }
            };
        }
        Ok(())
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
