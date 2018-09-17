use std::sync::RwLock;
use types::JavaType::*;
use java_class::attributes::Attribute;
use java_class::methods::MethodInfo;
use java_class::fields::FieldInfo;
use std::sync::Arc;
use std::collections::HashMap;
use java_class::class::JavaClass;
use jvm;
use java_class::cp_info::CPInfo;

/// wrapper for java_class::class::JavaClass that includes runtime data
#[derive(Debug)]
pub struct Class {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: SymbolicConstantPool,
    pub access_flags: u16,
    /// name in binary format
    pub name: String,
    /// may be `None` if and only if this is a primitive class or java/lang/Object
    pub super_class: Option<Arc<RwLock<Class>>>,
    pub interfaces: Vec<Arc<RwLock<Class>>>,
    pub fields: HashMap<String, Arc<RwLock<Field>>>,
    pub instance_fields: Vec<Arc<InstanceFieldInfo>>,
    pub methods: HashMap<String, Arc<RwLock<Method>>>,
    pub attributes: Vec<Attribute>
}

impl Class {
    pub fn new(class: JavaClass) -> Result<Class, ()> {
        let minor_version = class.minor_version;
        let major_version = class.major_version;
        let constant_pool = SymbolicConstantPool::new(&class.constant_pool)?;
        let access_flags = class.access_flags;
        let name = jvm::get_name_cp(&class.constant_pool, class.this_class);
        // if this is java/lang/Object it has no super class
        let super_class = if name=="java/lang/Object" {
            None
        } else {
            //shouldn't need to guard against circular superclassing since that's done while loading the .class in ::jvm
            Some(jvm::get_or_load_class(&jvm::get_name_cp(&class.constant_pool, class.super_class))?)
        };
        let mut interfaces = Vec::with_capacity(class.interfaces.len());
        for interface_index in &class.interfaces {
            //shouldn't need to guard against circular interfacing since that's done while loading the .class in ::jvm
            interfaces.push(jvm::get_or_load_class(&jvm::get_name_cp(&class.constant_pool, class.super_class))?);
        }
        let mut fields = HashMap::new();
        let mut instance_fields = vec!();
        for field in &class.fields {
            if field.access_flags & (::java_class::fields::AccessFlags::Static as u16) != 0 {
                //static field
                let field_n = Field::new(&class, &field);
                fields.insert(field_n.name.to_owned(), Arc::new(RwLock::new(field_n)));
            } else {
                let field_n = InstanceFieldInfo::new(&class, &field);
                instance_fields.push(Arc::new(field_n));
            }
        }
        let mut methods = HashMap::new();
        for method in &class.methods {
            let method_n = Method::new(&class, &method);
            methods.insert(method_n.repr.to_owned(), Arc::new(RwLock::new(method_n)));
        }
        let attributes = class.attributes.clone();
        Ok(Class { minor_version, major_version, constant_pool, access_flags, name,
                   super_class, interfaces, fields, instance_fields, methods, attributes })
    }

    pub fn is_interface(&self) -> bool {
        self.access_flags & (::java_class::class::AccessFlags::Interface as u16) != 0
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

impl SymbolicConstantPool {
    pub fn new(cp: &::java_class::cp::ConstantPool) -> Result<SymbolicConstantPool, ()> {
        let mut ans = vec!();
        for cp_info in cp.items() {
            let next = match cp_info {
                CPInfo::Class {name_index} => {
                    let name = jvm::get_name_cp(cp, *name_index);
                    SymbolicConstantPoolEntry::Class(jvm::get_or_load_class(&name)?)
                },
                CPInfo::Fieldref { class_index, name_and_type_index } => {
                    let class_name = jvm::get_name_cp(cp, *class_index);
                    let name_and_type = &cp[*name_and_type_index];
                    let (name, type_) = match name_and_type {
                        CPInfo::NameAndType { name_index, descriptor_index } => {
                            (jvm::get_name_cp(cp, *name_index),
                             jvm::get_name_cp(cp, *descriptor_index))
                        },
                        _ => panic!()
                    };
                    let class = jvm::get_or_load_class(&class_name)?;
                    SymbolicConstantPoolEntry::Fieldref { class, name, type_ }
                },
                CPInfo::Methodref { class_index, name_and_type_index } => {
                    let class_name = jvm::get_name_cp(cp, *class_index);
                    let name_and_type = &cp[*name_and_type_index];
                    let (name, type_) = match name_and_type {
                        CPInfo::NameAndType { name_index, descriptor_index } => {
                            (jvm::get_name_cp(cp, *name_index),
                             jvm::get_name_cp(cp, *descriptor_index))
                        },
                        _ => panic!()
                    };
                    let name = name+&type_;
                    let class = jvm::get_or_load_class(&class_name)?;
                    SymbolicConstantPoolEntry::Methodref { class, name }
                },
                CPInfo::InterfaceMethodref { class_index, name_and_type_index } => {
                    let class_name = jvm::get_name_cp(cp, *class_index);
                    let name_and_type = &cp[*name_and_type_index];
                    let (name, type_) = match name_and_type {
                        CPInfo::NameAndType { name_index, descriptor_index } => {
                            (jvm::get_name_cp(cp, *name_index),
                             jvm::get_name_cp(cp, *descriptor_index))
                        },
                        _ => panic!()
                    };
                    let name = name+&type_;
                    let class = jvm::get_or_load_class(&class_name)?;
                    SymbolicConstantPoolEntry::InterfaceMethodref { class, name }
                },
                CPInfo::String { string_index } => {
                    SymbolicConstantPoolEntry::String(Arc::new(jvm::get_name_cp(cp, *string_index)))
                },
                CPInfo::Integer { bytes } => SymbolicConstantPoolEntry::Integer(*bytes as i32),
                CPInfo::Float { bytes } => {
                    unsafe {
                        SymbolicConstantPoolEntry::Float(::std::mem::transmute::<u32, f32>(*bytes))
                    }
                },
                CPInfo::Long { bytes } => SymbolicConstantPoolEntry::Long(*bytes as i64),
                CPInfo::Double { bytes } => {
                    unsafe {
                        SymbolicConstantPoolEntry::Double(::std::mem::transmute::<u64, f64>(*bytes))
                    }
                },
                _ => SymbolicConstantPoolEntry::DummyEntry
            };
            match next {
                SymbolicConstantPoolEntry::Long {..} | 
                SymbolicConstantPoolEntry::Double {..} => {
                    ans.push(next);
                    ans.push(SymbolicConstantPoolEntry::DummyEntry);
                },
                _ => ans.push(next)
            }
        }
        Ok(SymbolicConstantPool { constant_pool: ans })
    }
    pub fn new_empty() -> SymbolicConstantPool {
        SymbolicConstantPool {
            constant_pool: vec!()
        }
    }
}

#[derive(Debug)]
/// versions of CPInfo variants, except any symbolic references have been resoved to runtime references
pub enum SymbolicConstantPoolEntry {
    Class(Arc<RwLock<Class>>),
    Fieldref { class: Arc<RwLock<Class>>, name: String, type_: String },
    Methodref { class: Arc<RwLock<Class>>, name: String },
    InterfaceMethodref { class: Arc<RwLock<Class>>, name: String },
    String(Arc<String>),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    DummyEntry
    //TODO: MethodHandle, MethodType, InvokeDynamic
}

/// runtime information for a field
#[derive(Debug)]
pub struct Field {
    /// the access flags of the field
    pub access_flags: u16,
    /// the name of the field
    pub name: String,
    /// the descriptor of the field in stripped ('L'-';' removed) binary representation
    pub descriptor: String,
    /// the descriptor in binary format
    pub descriptor_raw: String,
    /// the attributes of the field
    pub attributes: Vec<Attribute>,
    /// the value of the field
    pub value: Arc<RwLock<JavaType>>
}

impl Field {

    fn get_default_value(descriptor: &String) -> JavaType {
        let start = descriptor.chars().next().unwrap();
        match start {
            'L' | '[' => JavaType::Null,
            'B' => JavaType::Byte(0),
            'C' => JavaType::Char('\0'),
            'D' => JavaType::Double(0f64),
            'F' => JavaType::Float(0f32),
            'I' => JavaType::Int(0),
            'J' => JavaType::Long(0),
            'S' => JavaType::Short(0),
            'Z' => JavaType::Boolean(false),
            _ => panic!("Invalid field descriptor")
        }
    }

    fn new(class: &JavaClass, field_info: &FieldInfo) -> Field {
        let access_flags = field_info.access_flags;
        let name = jvm::get_name(class, &class.constant_pool[field_info.name_index]);
        let descriptor_raw = jvm::get_name(class, &class.constant_pool[field_info.descriptor_index]);
        let d_r_2 = descriptor_raw.to_owned();
        let mut descriptor_chars = d_r_2.chars();
        let descriptor = parse_type(descriptor_chars.next().unwrap(), &mut descriptor_chars);
        let attributes = field_info.attributes.clone();
        let value = Arc::new(RwLock::new(Field::get_default_value(&descriptor_raw)));
        Field { access_flags, name, descriptor_raw, descriptor, attributes, value }
    }

    fn from_instance_field_info(info: Arc<RwLock<InstanceFieldInfo>>) -> Field {
        let f = info.read().unwrap();
        let value = Arc::new(RwLock::new(Field::get_default_value(&f.descriptor_raw)));
        Field { access_flags: f.access_flags, name: f.name.to_owned(), descriptor_raw: f.descriptor_raw.to_owned(),
               descriptor: f.descriptor.to_owned(), attributes: f.attributes.clone(), value }
    }
}

/// used as a holder for information needed to create instances of classes
#[derive(Debug)]
pub struct InstanceFieldInfo {
    /// the access flags of the field
    pub access_flags: u16,
    /// the name of the field
    pub name: String,
    /// the descriptor of the field in stripped ('L'-';' removed) binary representation
    pub descriptor: String,
    /// the descriptor in binary format
    pub descriptor_raw: String,
    /// the attributes of the field
    pub attributes: Vec<Attribute>
}

impl InstanceFieldInfo {
    fn new(class: &JavaClass, field_info: &FieldInfo) -> InstanceFieldInfo {
        let access_flags = field_info.access_flags;
        let name = jvm::get_name(class, &class.constant_pool[field_info.name_index]);
        let descriptor_raw = jvm::get_name(class, &class.constant_pool[field_info.descriptor_index]);
        let d_r_2 = descriptor_raw.to_owned();
        let mut descriptor_chars = d_r_2.chars();
        let descriptor = parse_type(descriptor_chars.next().unwrap(), &mut descriptor_chars);
        let attributes = field_info.attributes.clone();
        InstanceFieldInfo {access_flags, name, descriptor_raw, descriptor, attributes}
    }
}

/// runtime information for a method
#[derive(Debug)]
pub struct Method {
    /// the name of the method
    pub name: String,
    /// the signature of the method in binary representation
    pub descriptor: String,
    /// the binary representation of the method and its signature
    pub repr: String,
    /// a list of the parameters of the method in binary format
    pub parameters: Vec<String>,
    /// the return type of the method in binary format
    pub return_type: String,
    /// the access flags of the methood
    pub access_flags: u16,
    /// the attributes of the function (including the Code attribute)
    pub attributes: Vec<Attribute>,
    /// the index of the attribute containing the bytecode of the method
    /// guaranteed to be Attribute::Code if this method is not native or abstract
    pub code_attr_index: usize
}

impl Method {
    pub fn new(class: &JavaClass, method_info: &MethodInfo) -> Method {
        let (parameters, return_type) = 
            parse_parameters_return(&jvm::get_name(class, &class.constant_pool[method_info.descriptor_index]));
        let name = jvm::get_name(class, &class.constant_pool[method_info.name_index]);
        let descriptor = jvm::get_name(class, &class.constant_pool[method_info.descriptor_index]);
        let repr = name.to_owned()+&descriptor;
        let access_flags = method_info.access_flags;
        let attributes = method_info.attributes.clone();
        //dirty use of a closure for early return, probably should be separate method
        let code_attr_index = (|| {
            for code_attr_index in 0..attributes.len() {
                let code_attr = &attributes[code_attr_index];
                match code_attr {
                    Attribute::Code {..} => return code_attr_index,
                    _ => {}
                }
            }
            //must be a native or abstract method
            0
        })();
        Method { name, descriptor, repr, parameters, return_type, access_flags, attributes, code_attr_index }
        
    }
}

#[derive(Debug)]
pub enum JavaType {
    Boolean(bool),
    Byte(u8),
    Short(i16),
    Char(char),
    Int(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    ClassInstance { class: Arc<RwLock<Class>>, fields: HashMap<String, JavaType> },
    Object(Arc<RwLock<JavaType>>),
    Null
}

impl ::std::cmp::PartialEq for JavaType {
    fn eq(&self, other: &JavaType) -> bool {
        match self {
            Boolean(val) => match other {
                Boolean(val2) => val==val2,
                _ => false
            }
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