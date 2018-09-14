use std::sync::Mutex;
use types::JavaType::*;
use java_class::attributes::Attribute;
use java_class::methods::MethodInfo;
use std::sync::Arc;
use std::collections::HashMap;
use java_class::class::JavaClass;
use jvm;

/// wrapper for java_class::class::JavaClass that includes runtime data
#[derive(Debug)]
pub struct Class {
    pub class: JavaClass,
    pub fields: HashMap<String, JavaType>
}

impl Class {
    pub fn new(class: JavaClass) -> Class {
        Class {
            class,
            fields: HashMap::<String, JavaType>::new()
        }
    }
}

fn parse_type(start: char, chars: &mut ::std::str::Chars) -> String {
    let mut ans = String::new();
    if start == '[' {
        ans.push('[');
        ans += &parse_type(chars.next().unwrap(), chars);
        ans
    } else if start=='L' {
        loop {
            let c = chars.next().unwrap();
            if c == ';' {
                break;
            }
            ans.push(c);
        }
        ans
    } else {
        ans.push(start);
        ans
    }
}

pub fn parse_parameters_return(signature: &String) -> (Vec<String>, String) {
    let mut ans = vec!();
    let mut chars = signature.chars();
    chars.next(); //skip starting '('
    loop {
        let c = chars.next().unwrap();
        if c == ')' {
            break;
        }
        ans.push(parse_type(c, &mut chars));
    }
    (ans, parse_type(chars.next().unwrap(), &mut chars))
}

/// constant pool using references to runtime JVM information
#[derive(Debug)]
pub struct SymbolicConstantPool {
    constant_pool: Vec<SymbolicConstantPoolEntry>
}

#[derive(Debug)]
/// versions of CPInfo variants, except any symbolic references have been resoved to runtime references
pub enum SymbolicConstantPoolEntry {
    Class(Arc<Mutex<Class>>),
    Fieldref { class: Arc<Mutex<Class>>, name: String, type_: String },
    Methodref { class: Arc<Mutex<Class>>, name: String, type_: String},
    InterfaceMethodref { class: Arc<Mutex<Class>>, name: String, type_: String},
    String(Arc<Mutex<JavaType>>),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    //TODO: MethodHandle, MethodType, InvokeDynamic
}

/// wrapper for java_class::methods::MethodInfo
#[derive(Debug)]
pub struct Method<'a> {
    /// underlying .class representation
    pub method_info: &'a MethodInfo,
    /// the name of the method
    pub name: String,
    /// the signature of the method in binary representation
    pub descriptor: String,
    /// the binary representation of the method and its signature
    pub repr: String,
    /// a list of the parameters of the function in binary format
    pub parameters: Vec<String>,
    /// the return type of the function in binary format
    pub return_type: String,
    /// the attribute containing the bytecode of the method, guaranteed to be Attribute::Code
    pub code_attr: &'a Attribute
}

impl<'a> Method<'a> {
    pub fn new(class: &JavaClass, method_info: &'a MethodInfo) -> Method<'a> {
        let (parameters, return_type) = 
            parse_parameters_return(&jvm::get_name(class, &class.constant_pool[method_info.descriptor_index]));
        let name = jvm::get_name(class, &class.constant_pool[method_info.name_index]);
        let descriptor = jvm::get_name(class, &class.constant_pool[method_info.descriptor_index]);
        let repr = name.to_owned()+&descriptor;
        for code_attr in &method_info.attributes {
            match code_attr {
                Attribute::Code {..} => return Method { method_info, name, descriptor, repr, parameters, return_type, code_attr },
                _ => {}
            }
        };
        panic!("Missing code attribute");
    }
}

#[derive(Debug)]
pub enum JavaType {
    Byte(u8),
    Short(i16),
    Char(char),
    Int(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    ClassInstance { class: Arc<Mutex<Class>>, fields: HashMap<String, JavaType> },
    Object(Arc<Mutex<JavaType>>),
    Null
}

impl ::std::cmp::PartialEq for JavaType {
    fn eq(&self, other: &JavaType) -> bool {
        match self {
            Byte(val) => match other {
                Byte(val2) => val==val2,
                _ => false
            },
            Short(val) => match other {
                Short(val2) => val==val2,
                _ => false
            },
            Char(val) => match other {
                Char(val2) => val==val2,
                _ => false
            },
            Int(val) => match other {
                Int(val2) => val==val2,
                _ => false
            },
            Float(val) => match other {
                Float(val2) => val==val2,
                _ => false
            },
            Long(val) => match other {
                Long(val2) => val==val2,
                _ => false
            },
            Double(val) => match other {
                Double(val2) => val==val2,
                _ => false
            },
            ClassInstance {..} => false, //should never be called on 2 ClassInstances
            Object(a) => match other {
                Object(b) => Arc::ptr_eq(a, b),
                _ => false
            },
            Null => match other {
                Null => true,
                _ => false
            }
        }
    }
}