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
    pub fn new() -> Class {
        Class {
            minor_version: 0, major_version: 0, constant_pool: SymbolicConstantPool::new_empty(),
            access_flags: 0, name: "".to_string(), super_class: None, interfaces: vec!(), fields: HashMap::new(),
            instance_fields: vec!(), methods: HashMap::new(), attributes: vec!()
        }
    }

    pub fn initialize_start(&self, class: &JavaClass) -> Result<(), ()> {
        for cp_info in class.constant_pool.items() {
            match cp_info {
                CPInfo::Class {name_index} => {
                    let name = jvm::get_name_cp(&class.constant_pool, *name_index);
                    jvm::add_to_load(&name);
                },
                _ => {}
            };
        }
        Ok(())
    }

    pub fn initialize(&mut self, class: &Box<JavaClass>) -> Result<(), ()> {
        if self.name.len() > 0 && &self.name[..1]=="[" {
            //array class, initialize access flags
            let mut sub_name = self.name[1..].to_string();
            if &sub_name[..1]=="L" {
                let x = sub_name[1..sub_name.len()-1].to_owned();
                sub_name = x;
            } else {
                //it's an array of primitives
                self.access_flags = ::java_class::class::AccessFlags::Public as u16;
                return Ok(());
            }
            //unwrap should be ok since the subclass goes ahead of the array class in initialization order
            let subclass = jvm::get_class(&sub_name).unwrap();
            self.access_flags = subclass.read().unwrap().access_flags & ::java_class::class::AccessFlags::Public as u16;
            return Ok(());
        }
        debug!("Initializing class");
        self.minor_version = class.minor_version;
        self.major_version = class.major_version;
        self.constant_pool = SymbolicConstantPool::new(&class.constant_pool)?;
        self.access_flags = class.access_flags;
        self.name = jvm::get_name_cp(&class.constant_pool, class.this_class);
        
        // if this is java/lang/Object it has no super class
        self.super_class = if self.name=="java/lang/Object" || class.super_class==0 {
            None
        } else {
            //shouldn't need to guard against circular superclassing since that's done while loading the .class in ::jvm
            let ans = jvm::get_class(&jvm::get_name_cp(&class.constant_pool, class.super_class)).unwrap();
            Some(ans)
        };
        self.interfaces = Vec::with_capacity(class.interfaces.len());
        for interface_index in &class.interfaces {
            //shouldn't need to guard against circular interfacing since that's done while loading the .class in ::jvm
            let ans = jvm::get_class(&jvm::get_name_cp(&class.constant_pool, *interface_index)).unwrap();
            self.interfaces.push(ans);
        }
        self.fields = HashMap::new();
        self.instance_fields = vec!();
        for field in &class.fields {
            if field.access_flags & (::java_class::fields::AccessFlags::Static as u16) != 0 {
                //static field
                let field_n = Field::new(&class, &field);
                self.fields.insert(field_n.name.to_owned(), Arc::new(RwLock::new(field_n)));
            } else {
                let field_n = InstanceFieldInfo::new(&class, &field);
                self.instance_fields.push(Arc::new(field_n));
            }
        }
        self.methods = HashMap::new();
        for method in &class.methods {
            let method_n = Method::new(&class, &method)?;
            self.methods.insert(method_n.repr.to_owned(), Arc::new(RwLock::new(method_n)));
        }
        self.attributes = class.attributes.clone();
        Ok(())
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
            //debug!("CP item {:?}", cp_info);
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
        debug!("CP Done");
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

    pub fn new(class: &JavaClass, field_info: &FieldInfo) -> Field {
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

    pub fn from_instance_field_info(info: Arc<RwLock<InstanceFieldInfo>>) -> Field {
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
    pub fn new(class: &JavaClass, method_info: &MethodInfo) -> Result<Method, ()> {
        let (parameters, return_type) = 
            parse_parameters_return(&jvm::get_name(class, &class.constant_pool[method_info.descriptor_index]));
        let name = jvm::get_name(class, &class.constant_pool[method_info.name_index]);
        let descriptor = jvm::get_name(class, &class.constant_pool[method_info.descriptor_index]);
        let repr = name.to_owned()+&descriptor;
        let access_flags = method_info.access_flags;
        let attributes = method_info.attributes.clone();
        //dirty use of a closure for early return, probably should be separate method
        let code_attr_index = if method_info.is_abstract() || method_info.is_native() {
            //abstract and native methods have no code attribute
            Ok(0)
        } else {
            (|| {
                for code_attr_index in 0..attributes.len() {
                    let code_attr = &attributes[code_attr_index];
                    match code_attr {
                        Attribute::Code {..} => return Ok(code_attr_index),
                        _ => {}
                    }
                }
                Err(())
            })()
        }?;
        Ok(Method { name, descriptor, repr, parameters, return_type, access_flags, attributes, code_attr_index })
        
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