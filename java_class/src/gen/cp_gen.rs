#![allow(dead_code)]

use cp_info::CPInfo;
use std::collections::HashMap;
//TODO: MethodHandle, MethodType, and InvokeDynamic
pub struct CPGen {
    items: Vec<CPInfo>,
    string_table: HashMap<String, u16>,
    class_table: HashMap<String, u16>,
    utf8_table: HashMap<String, u16>,
    name_type_table: HashMap<String, u16>, //name%type
    fieldref_table: HashMap<String, u16>, //class%name%type
    methodref_table: HashMap<String, u16>, //class%name%type
    imethodref_table: HashMap<String, u16>, //class%name%type

}

impl CPGen {

    pub fn new() -> CPGen {
        CPGen {
            items: vec!(),
            string_table: HashMap::new(),
            class_table: HashMap::new(),
            utf8_table: HashMap::new(),
            name_type_table: HashMap::new(),
            fieldref_table: HashMap::new(),
            methodref_table: HashMap::new(),
            imethodref_table: HashMap::new(),
        }
    }

    pub fn add_integer(&mut self, val: u32) -> u16 {
        for i in 0..self.items.len() {
            if let CPInfo::Integer { bytes } = &self.items[i] {
                if *bytes==val {
                    return i as u16;
                }
            }
        }
        let ind = self.items.len() as u16;
        self.items.push(CPInfo::Integer { bytes: val });
        ind
    }

    pub fn add_long(&mut self, val: u64) -> u16 {
        for i in 0..self.items.len() {
            if let CPInfo::Long { bytes } = &self.items[i] {
                if *bytes==val {
                    return i as u16;
                }
            }
        }
        let ind = self.items.len() as u16;
        self.items.push(CPInfo::Long { bytes: val });
        self.items.push(CPInfo::LongDoubleDummy);
        ind
    }

    pub fn add_float(&mut self, val: u32) -> u16 {
        for i in 0..self.items.len() {
            if let CPInfo::Float { bytes } = &self.items[i] {
                if *bytes==val {
                    return i as u16;
                }
            }
        }
        let ind = self.items.len() as u16;
        self.items.push(CPInfo::Float { bytes: val });
        ind
    }

    pub fn add_float_f32(&mut self, val: f32) -> u16 {
        self.add_float(val.to_bits())
    }

    pub fn add_double(&mut self, val: u64) -> u16 {
        for i in 0..self.items.len() {
            if let CPInfo::Double { bytes } = &self.items[i] {
                if *bytes==val {
                    return i as u16;
                }
            }
        }
        let ind = self.items.len() as u16;
        self.items.push(CPInfo::Double { bytes: val });
        self.items.push(CPInfo::LongDoubleDummy);
        ind
    }

    pub fn add_double_f64(&mut self, val: f64) -> u16 {
        self.add_double(val.to_bits())
    }

    pub fn add_string(&mut self, s: String) -> u16 {
        #[allow(clippy::clippy::map_entry)] //since we modify `self`, `.entry().or_insert()` is a pain
        if self.string_table.contains_key(&s) {
            self.string_table[&s]
        } else {
            let utf8_index = self.add_utf8(s.to_owned());
            let ind = self.items.len() as u16;
            self.items.push(CPInfo::String { string_index: utf8_index });
            self.string_table.insert(s, ind);
            ind
        }
    }

    pub fn add_class(&mut self, name: String) -> u16 {
        #[allow(clippy::clippy::map_entry)] //since we modify `self`, `.entry().or_insert()` is a pain
        if self.class_table.contains_key(&name) {
            self.class_table[&name]
        } else {
            let utf8_index = self.add_utf8(name.to_owned());
            let ind = self.items.len() as u16;
            self.items.push(CPInfo::Class { name_index: utf8_index });
            self.class_table.insert(name, ind);
            ind
        }
    }

    pub fn add_name_type(&mut self, name: String, type_: String) -> u16 {
        let key = name.to_owned()+"%"+&type_;
        if self.class_table.contains_key(&key) {
            self.class_table[&key]
        } else {
            let name_index = self.add_utf8(name);
            let type_index = self.add_utf8(type_);
            let ind = self.items.len() as u16;
            self.items.push(CPInfo::NameAndType { name_index, descriptor_index: type_index });
            self.name_type_table.insert(key, ind);
            ind
        }
    }

    pub fn add_fieldref(&mut self, c_name: String, name: String, type_: String) -> u16 {
        let key = c_name.to_owned()+"%"+&name+"%"+&type_;
        #[allow(clippy::clippy::map_entry)] //since we modify `self`, `.entry().or_insert()` is a pain
        if self.fieldref_table.contains_key(&key) {
            self.fieldref_table[&key]
        } else {
            let name_and_type_index = self.add_name_type(name, type_);
            let class_index = self.add_class(c_name);
            let ind = self.items.len() as u16;
            self.items.push(CPInfo::Fieldref { class_index, name_and_type_index });
            self.fieldref_table.insert(key, ind);
            ind
        }
    }

    pub fn add_methodref(&mut self, c_name: String, name: String, type_: String) -> u16 {
        let key = c_name.to_owned()+"%"+&name+"%"+&type_;
        #[allow(clippy::clippy::map_entry)] //since we modify `self`, `.entry().or_insert()` is a pain
        if self.methodref_table.contains_key(&key) {
            self.methodref_table[&key]
        } else {
            let name_and_type_index = self.add_name_type(name, type_);
            let class_index = self.add_class(c_name);
            let ind = self.items.len() as u16;
            self.items.push(CPInfo::Methodref { class_index, name_and_type_index });
            self.methodref_table.insert(key, ind);
            ind
        }
    }

    pub fn add_interface_methodref(&mut self, c_name: String, name: String, type_: String) -> u16 {
        let key = c_name.to_owned()+"%"+&name+"%"+&type_;
        #[allow(clippy::clippy::map_entry)] //since we modify `self`, `.entry().or_insert()` is a pain
        if self.imethodref_table.contains_key(&key) {
            self.imethodref_table[&key]
        } else {
            let name_and_type_index = self.add_name_type(name, type_);
            let class_index = self.add_class(c_name);
            let ind = self.items.len() as u16;
            self.items.push(CPInfo::InterfaceMethodref { class_index, name_and_type_index });
            self.imethodref_table.insert(key, ind);
            ind
        }
    }

    pub fn add_utf8(&mut self, s: String) -> u16 {
        #[allow(clippy::clippy::map_entry)] //since we modify `self`, `.entry().or_insert()` is a pain
        if self.utf8_table.contains_key(&s) {
            self.utf8_table[&s]
        } else {
            let ind = self.items.len() as u16;
            self.items.push(CPInfo::Utf8 { length: s.len() as u16, bytes: s.as_bytes().to_vec() });
            self.utf8_table.insert(s, ind);
            ind
        }
    }

}