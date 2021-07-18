use java_class::attributes::Attribute;
use java_class::class::JavaClass;
use java_class::opcodes::Opcode;
use java_class::cp_info::CPInfo;
use java_class::fields::FieldInfo;
use java_class::methods::MethodInfo;
use jvm;
use std::collections::HashMap;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ops::Index;
use std::sync::Arc;
use std::sync::RwLock;
use types::JavaType::*;

pub type ClassRef = &'static Class;

/// wrapper for java_class::class::JavaClass that includes runtime data
#[derive(Debug)]
pub struct Class {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: RuntimeConstantPool,
    pub access_flags: u16,
    /// name in binary format
    pub name: String,
    /// may be `None` if and only if this is a primitive class or java/lang/Object
    pub super_class: Option<ClassRef>,
    pub interfaces: Vec<ClassRef>,
    pub fields: HashMap<String, Field>,
    pub instance_fields: Vec<InstanceFieldInfo>,
    pub methods: HashMap<String, &'static Method>,
    pub attributes: Vec<Attribute>,
    pub array_inner: Option<ClassRef>
}

impl Default for Class {
    fn default() -> Class {
        Class {
            minor_version: 0,
            major_version: 0,
            constant_pool: RuntimeConstantPool::new_empty(),
            access_flags: 0,
            name: "".to_string(),
            super_class: None,
            interfaces: vec!(),
            fields: HashMap::new(),
            instance_fields: vec!(),
            methods: HashMap::new(),
            attributes: vec!(),
            array_inner: None
        }
    }
}

impl Class {

    pub fn initialize_start(&self, class: &JavaClass) -> Option<()> {
        for cp_info in class.constant_pool.items() {
            if let CPInfo::Class { name_index } = cp_info {
                let name = jvm::get_name_cp(&class.constant_pool, *name_index);
                jvm::add_to_load(&name);
            } 
        }
        Some(())
    }

    pub fn initialize(&mut self, class: &JavaClass, self_ref: ClassRef) -> Option<()> {
        if !self.name.is_empty() && &self.name[..1] == "[" {
            //array class, initialize access flags
            let mut sub_name = self.name[1..].to_string();
            if &sub_name[..1] == "L" {
                let x = sub_name[1..sub_name.len() - 1].to_owned();
                sub_name = x;
            } else {
                //it's an array of primitives
                self.access_flags = ::java_class::class::AccessFlags::Public as u16;
                return Some(());
            }
            //unwrap should be ok since the subclass goes ahead of the array class in initialization order
            let subclass = jvm::get_class(&sub_name).unwrap();
            self.access_flags = subclass.access_flags & ::java_class::class::AccessFlags::Public as u16;
            self.array_inner = Some(subclass);
            return Some(());
        }
        debug!("Initializing class");
        self.minor_version = class.minor_version;
        self.major_version = class.major_version;
        self.constant_pool = RuntimeConstantPool::new(&class.constant_pool)?;
        self.access_flags = class.access_flags;
        self.name = jvm::get_name_cp(&class.constant_pool, class.this_class);

        // if this is java/lang/Object it has no super class
        self.super_class = if self.name == "java/lang/Object" || class.super_class == 0 {
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
                self.fields.insert(field_n.name.to_owned(), field_n);
            } else {
                let field_n = InstanceFieldInfo::new(&class, &field);
                self.instance_fields.push(field_n);
            }
        }
        self.methods = HashMap::new();
        for method in &class.methods {
            let method_n = Method::new(self_ref, &class, &method)?;
            self.methods.insert(method_n.repr.to_owned(), Box::leak(Box::new(method_n)));
        }
        self.attributes = class.attributes.clone();
        Some(())
    }

    pub fn is_interface(&self) -> bool {
        self.access_flags & (::java_class::class::AccessFlags::Interface as u16) != 0
    }

    pub fn is_array(&self) -> bool {
        self.name.starts_with('[')
    }

    pub fn get_package(&self) -> &str {
        let mut ind = None;
        let bytes = self.name.as_bytes();
        for (i, &c) in bytes.iter().enumerate() {
            if c == b'/' {
                ind = Some(i);
                break;
            }
        }
        if let Some(ind) = ind {
            &self.name[..ind]
        } else {
            ""
        }
    }

    /// Goes up the superclass chain to check if `expected` is a superclass of self
    fn is_subclass_of(&self, expected: &Class) -> bool {
        let mut curr = Some(self);
        while curr.is_some() {
            let c = curr.unwrap();
            if c == expected {
                return true;
            }
            curr = c.super_class;
        }
        false
    }

    /// Checks if this class/interface, or any of its superinterfaces, implements `expected`
    pub fn implements(&self, expected: &Class) -> bool {
        self == expected || self.interfaces.iter().any(|c| {c.implements(expected)})
    }

    /// Returns true if this class is java.lang.Object
    pub fn is_object(&self) -> bool {
        self.super_class.is_none()
    }

    /// Returns true if `self instanceof expected` (if this class is an instance of `expected`)
    pub fn instanceof(&self, expected: &Class) -> bool {
        if self.is_interface() {
            if expected.is_interface() {
                self.implements(expected)
            } else {
                expected.is_object()
            }
        } else if self.is_array() {
            if expected.is_interface() {
                expected.name == "java/lang/Cloneable" || expected.name == "java/io/Serializable"
            } else if expected.is_array() {
                if self.array_inner.is_some() && expected.array_inner.is_some() {
                    let self_inner = self.array_inner.unwrap();
                    let expected_inner = expected.array_inner.unwrap();
                    self_inner.instanceof(expected_inner)
                } else {
                    self.name == expected.name
                }
            } else {
                expected.is_object()
            }
        } else if expected.is_interface() {
            self.instanceof(expected)
        } else {
            self.is_subclass_of(expected)
        }
    }

    pub fn get_default_value(&self) -> JavaType {
        match self.name.as_str() {
            "byte" => JavaType::Byte(0),
            "char" => JavaType::Char(0),
            "double" => JavaType::Double(0f64),
            "float" => JavaType::Float(0f32),
            "int" => JavaType::Int(0),
            "long" => JavaType::Long(0),
            "short" => JavaType::Short(0),
            "boolean" => JavaType::Boolean(false),
            _ => JavaType::Null
        }
    }

    fn find_method_superinterface(&self, repr: &str) -> Result<Option<&'static Method>, &str> {
        let mut found = None;
        for &i in &self.interfaces {
            if let Some(&m) = i.methods.get(repr) {
                use java_class::methods::AccessFlags;
                if !m.is_abstract() && m.access_flags & (AccessFlags::Private as u16 | AccessFlags::Static as u16) == 0 {
                    if found.is_some() {
                        return Err("IncompatibleClassChangeError");
                    } else {
                        found = Some(m)
                    }
                }
            }
        }
        Ok(found)
    }

    /// Resolves a method. Returns the name of an Exception if resolution fails.
    pub fn resolve_method(&self, name: &str, descriptor: &str) -> Result<&'static Method, &str> {
        //5.4.3.3. Method Resolution
        //Let C be the class containing the method we wish to resolve.
        //1. If C is an interface, method resolution throws an IncompatibleClassChangeError.
        if self.is_interface() {
            return Err("IncompatibleClassChangeError");
        }
        //2. Otherwise, method resolution attempts to locate the referenced method in C and its superclasses:
        let repr = name.to_owned() + descriptor;
        //TODO: If C declares exactly one method with the name specified by the method reference, and the declaration
        //is a signature polymorphic method, then method lookup succeeds.
        //TODO: All the class names mentioned in the descriptor are resolved.

        //Otherwise, if C declares a method with the name and descriptor specified by the method reference, method lookup succeeds.
        if let Some(m) = self.methods.get(&repr) {
            return Ok(m);
        }
        //Otherwise, if C has a superclass, step 2 of method resolution is recursively invoked on the direct superclass of C.
        let mut curr = self.super_class;
        while curr.is_some() {
            let c = curr.unwrap();
            if let Some(m) = c.methods.get(&repr) {
                return Ok(m);
            }
            curr = c.super_class;
        }
        //Otherwise, method resolution attempts to locate the referenced method in the superinterfaces of the specified class C:
        self.find_method_superinterface(&repr)?.ok_or("NoSuchMethodError")
    }

    /// Resolves a static method. Returns the name of an Exception if resolution fails.
    pub fn resolve_static_method(&self, name: &str, descriptor: &str) -> Result<&'static Method, &str> {
        //5.4.3.3. Method Resolution
        //Let C be the class containing the method we wish to resolve.
        //1. If C is an interface, method resolution throws an IncompatibleClassChangeError.
        if self.is_interface() {
            return Err("IncompatibleClassChangeError");
        }
        //2. Otherwise, method resolution attempts to locate the referenced method in C and its superclasses:
        let repr = name.to_owned() + descriptor;
        //TODO: If C declares exactly one method with the name specified by the method reference, and the declaration
        //is a signature polymorphic method, then method lookup succeeds.
        //TODO: All the class names mentioned in the descriptor are resolved.

        //Otherwise, if C declares a method with the name and descriptor specified by the method reference, method lookup succeeds.
        if let Some(m) = self.methods.get(&repr) {
            return Ok(m);
        }
        //Otherwise, if C has a superclass, step 2 of method resolution is recursively invoked on the direct superclass of C.
        let mut curr = self.super_class;
        while curr.is_some() {
            let c = curr.unwrap();
            if let Some(m) = c.methods.get(&repr) {
                return Ok(m);
            }
            curr = c.super_class;
        }
        //Otherwise, method resolution attempts to locate the referenced method in the superinterfaces of the specified class C:
        self.find_method_superinterface(&repr)?.ok_or("NoSuchMethodError")
    }

    /// Resolves an interface method. Returns the name of an Exception if resolution fails.
    pub fn resolve_interface_method(&self, name: &str, descriptor: &str) -> Result<&'static Method, &str> {
        //5.4.3.4. Interface Method Resolution
        //1. If C is not an interface, interface method resolution throws an IncompatibleClassChangeError.
        if !self.is_interface() {
            return Err("IncompatibleClassChangeError");
        }
        let repr = name.to_owned() + descriptor;
        //2. Otherwise, if C declares a method with the name and descriptor specified by the 
        //interface method reference, method lookup succeeds.
        if let Some(&m) = self.methods.get(&repr) {
            return Ok(m);
        }
        //3. Otherwise, if the class Object declares a method with the name and descriptor specified 
        //by the interface method reference, which has its ACC_PUBLIC flag set and does not have 
        //its ACC_STATIC flag set, method lookup succeeds.
        let object = jvm::get_class("java/lang/Object").unwrap();
        if let Some(&m) = object.methods.get(&repr) {
            if m.access_flags & (AccessFlags::Public as u16 | AccessFlags::Abstract as u16 | AccessFlags::Static as u16) == AccessFlags::Public as u16 {
                return Ok(m);
            }
        }
        //4. Otherwise, if the maximally-specific superinterface methods of C for the name and 
        //descriptor specified by the method reference include exactly one method that does 
        //not have its ACC_ABSTRACT flag set, then this method is chosen and method lookup succeeds
        let mut methods = vec!();
        let mut interfaces = self.interfaces.clone();
        while let Some(i) = interfaces.pop() {
            interfaces.extend(i.interfaces.iter());
            if let Some(&m) = i.methods.get(&repr) {
                if !m.is_abstract() {
                    methods.push(m);
                }
            }
        }
        if methods.len() == 1 {
            return Ok(methods[0]);
        }
        //5. Otherwise, if any superinterface of C declares a method with the name and descriptor 
        //specified by the method reference that has neither its ACC_PRIVATE flag nor its 
        //ACC_STATIC flag set, one of these is arbitrarily chosen and method lookup succeeds.
        use java_class::methods::AccessFlags;
        if let Some(m) = methods.iter().find(|c| {c.access_flags & (AccessFlags::Private as u16 | AccessFlags::Static as u16) == 0}) {
            return Ok(m);
        }
        //6. Otherwise, method lookup fails.
        Err("NoSuchMethodError")
    }

    /// This function should only be used in `invokespecial` after normal method resolution.
    pub fn resolve_method_invokespecial(&self, name: &str, descriptor: &str) -> Result<&'static Method, &str> {
        //this procedure is taken from the description of invokespecial
        let repr = name.to_owned() + descriptor;
        let c = if !name.ends_with("/<init>") && !self.is_interface() && (self.access_flags & java_class::class::AccessFlags::Super as u16 != 0) {
            self.super_class.unwrap()
        } else {
            self
        };
        if let Some(&m) = c.methods.get(&repr) {
            return Ok(m);
        }
        if !c.is_interface() {
            let mut curr = c.super_class;
            while let Some(c) = curr {
                if let Some(&m) = c.methods.get(&repr) {
                    return Ok(m);
                }
                curr = c.super_class;
            }
        } else {
            let object = jvm::get_class("java/lang/Object").unwrap();
            if let Some(&m) = object.methods.get(&repr) {
                use java_class::methods::AccessFlags;
                //Otherwise, if C is an interface and the class Object contains a declaration of a 
                //public instance method with the same name and descriptor as the resolved method, 
                //then it is the method to be invoked.
                if m.access_flags & (AccessFlags::Public as u16 | AccessFlags::Abstract as u16 | AccessFlags::Static as u16) == AccessFlags::Public as u16 {
                    return Ok(m);
                }
            }
        }
        self.find_method_superinterface(&repr)?.ok_or("AbstractMethodError")
    }

    fn find_override(&self, repr: &str, overridden: &'static Method) -> Option<&'static Method> {
        //if overridden wasn't declared in a superclass of self, then no method in self can
        //override it.
        {
            let mut curr = Some(self);
            let mut ok = false;
            while let Some(c) = curr {
                if c == overridden.class {
                    ok = true;
                    break;
                }
                curr = c.super_class;
            }
            if !ok {
                return None;
            }
        }
        //now we know that at least overridden can override itself (i.e., that it was declared in self
        //or in a superclass of self), so we're definitely returning Some.
        let mut best = overridden;
        let mut may_override = vec![overridden];
        let mut class_chain = vec!();
        {
            let mut curr = self;
            while curr != overridden.class {
                class_chain.push(curr);
                curr = curr.super_class.unwrap();
            }
        }
        class_chain.reverse();
        for c in class_chain {
            if let Some(&m) = c.methods.get(repr) {
                let mut ok = false;
                for &m2 in may_override.iter() {
                    if !m.is_private() {
                        let pub_prot = m2.is_public() || m2.is_protected();
                        if pub_prot || (!m2.is_private() && m.class.get_package() == m2.class.get_package()) {
                            ok = true;
                            break;
                        }
                    }
                }
                if ok {
                    best = m;
                    may_override.push(m);
                }
            }
        }
        Some(best)
    }

    /// This function should only be used in `invokevirtual` after normal method resolution.
    pub fn resolve_method_invokevirtual(&self, name: &str, descriptor: &str, resolved: &'static Method) -> Result<&'static Method, &str> {
        //this procedure is taken from the description of invokevirtual
        let repr = name.to_owned() + descriptor;
        if let Some(m) = self.find_override(&repr, resolved) {
            if m.is_abstract() {
                Err("AbstractMethodError")
            } else {
                Ok(m)
            }
        } else {
            self.find_method_superinterface(&repr)?.ok_or("AbstractMethodError")
        }
        
    }

    /// This method should only be used in `invokeinterface` after normal method resolution,
    pub fn resolve_method_invokeinterface(&self, name: &str, descriptor: &str) -> Result<&'static Method, &str> {
        //this procedure is taken from the description of invokeinterface
        use java_class::methods::AccessFlags;
        let repr = name.to_owned() + descriptor;
        if let Some(m) = self.methods.get(&repr) {
            if m.access_flags & AccessFlags::Public as u16 == 0 {
                return Err("IllegalAccessError");
            }
            if m.is_abstract() {
                return Err("AbstractMethodError");
            }
            return Ok(m);
        }
        let mut curr = self.super_class;
        while curr.is_some() {
            let c = curr.unwrap();
            if let Some(m) = c.methods.get(&repr) {
                if m.access_flags & AccessFlags::Public as u16 == 0 {
                    return Err("IllegalAccessError");
                }
                if m.is_abstract() {
                    return Err("AbstractMethodError");
                }
                return Ok(m);
            }
            curr = c.super_class;
        }
        let mut ans = None;
        for i in &self.interfaces {
            if let Some(m) = i.methods.get(&repr) {
                if !m.is_abstract() && m.access_flags & (AccessFlags::Private as u16 | AccessFlags::Static as u16) == 0 {
                    if ans.is_none() {
                        ans = Some(*m)
                    } else {
                        return Err("IncompatibleClassChangeError");
                    }
                }
            }
        }
        ans.ok_or("AbstractMethodError")
    }

    pub fn instantiate(&'static self) -> JavaType {
        let obj = self.create_obj();
        jvm::add_to_gc(obj.clone());
        JavaType::Reference {class: self, val: obj}
    }

    pub fn instantiate_no_gc(&'static self) -> JavaType {
        JavaType::Reference {class: self, val: self.create_obj()}
    }

    fn create_obj(&'static self) -> Arc<RwLock<JavaType>> {
        let mut fields = HashMap::new();
        let mut curr = self;
        loop {
            for f in &self.instance_fields {
                if !fields.contains_key(&f.name) {
                    fields.insert(f.name.clone(), f.class.get_default_value());
                }
            }
            if let Some(c) = curr.super_class {
                curr = c;
            } else {
                break;
            }
        }
        Arc::new(RwLock::new(JavaType::Object {class: self, fields}))
    }

    /// Gets the instance of java.lang.Class representing this class.
    pub fn get_class_obj(&'static self) -> JavaType {
        let class_class = jvm::get_or_load_class("java/lang/Class").unwrap();
        let ans = class_class.instantiate();
        ans.set_field("class", JavaType::Object {class: self, fields: HashMap::new()});
        ans
    }

}

impl PartialEq for Method {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl PartialEq for Class {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

fn parse_type(start: char, chars: &mut ::std::str::Chars) -> String {
    let mut ans = String::new();
    if start == '[' {
        ans.push('[');
        ans += &parse_type(chars.next().unwrap(), chars);
        ans
    } else if start == 'L' {
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

pub fn parse_parameters_return(signature: &str) -> (Vec<String>, String) {
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
/// 1-indexed, as per JVM specification
#[derive(Debug)]
pub struct RuntimeConstantPool {
    pub constant_pool: Vec<RuntimeConstantPoolEntry>
}

impl RuntimeConstantPool {
    pub fn new(cp: &::java_class::cp::ConstantPool) -> Option<RuntimeConstantPool> {
        let mut ans = vec!();
        for cp_info in cp.items() {
            //debug!("CP item {:?}", cp_info);
            let next = match cp_info {
                CPInfo::Class { name_index } => {
                    let name = jvm::get_name_cp(cp, *name_index);
                    RuntimeConstantPoolEntry::Class(jvm::get_or_load_class(&name)?)
                }
                CPInfo::Fieldref { class_index, name_and_type_index } => {
                    let class_name = jvm::get_name_cp(cp, *class_index);
                    let name_and_type = &cp[*name_and_type_index];
                    let (name, type_) = match name_and_type {
                        CPInfo::NameAndType { name_index, descriptor_index } => {
                            (jvm::get_name_cp(cp, *name_index),
                             jvm::get_name_cp(cp, *descriptor_index))
                        }
                        _ => panic!()
                    };
                    let class = jvm::get_or_load_class(&class_name)?;
                    RuntimeConstantPoolEntry::Fieldref { class, name, type_: jvm::get_or_load_class(&type_)? }
                }
                CPInfo::Methodref { class_index, name_and_type_index } => {
                    let class_name = jvm::get_name_cp(cp, *class_index);
                    let name_and_type = &cp[*name_and_type_index];
                    let (name, descriptor) = match name_and_type {
                        CPInfo::NameAndType { name_index, descriptor_index } => {
                            (jvm::get_name_cp(cp, *name_index),
                             jvm::get_name_cp(cp, *descriptor_index))
                        }
                        _ => panic!()
                    };
                    let class = jvm::get_or_load_class(&class_name)?;
                    RuntimeConstantPoolEntry::Methodref { class, name, descriptor }
                }
                CPInfo::InterfaceMethodref { class_index, name_and_type_index } => {
                    let class_name = jvm::get_name_cp(cp, *class_index);
                    let name_and_type = &cp[*name_and_type_index];
                    let (name, descriptor) = match name_and_type {
                        CPInfo::NameAndType { name_index, descriptor_index } => {
                            (jvm::get_name_cp(cp, *name_index),
                             jvm::get_name_cp(cp, *descriptor_index))
                        }
                        _ => panic!()
                    };
                    let class = jvm::get_or_load_class(&class_name)?;
                    RuntimeConstantPoolEntry::InterfaceMethodref { class, name, descriptor }
                }
                CPInfo::String { string_index } => {
                    RuntimeConstantPoolEntry::String(jvm::get_or_intern_string(jvm::get_name_cp(cp, *string_index)))
                }
                CPInfo::Integer { bytes } => RuntimeConstantPoolEntry::Integer(*bytes as i32),
                CPInfo::Float { bytes } => {
                    RuntimeConstantPoolEntry::Float(f32::from_bits(*bytes))
                }
                CPInfo::Long { bytes } => RuntimeConstantPoolEntry::Long(*bytes as i64),
                CPInfo::Double { bytes } => {
                    RuntimeConstantPoolEntry::Double(f64::from_bits(*bytes))
                }
                _ => RuntimeConstantPoolEntry::DummyEntry
            };
            match next {
                RuntimeConstantPoolEntry::Long { .. } |
                RuntimeConstantPoolEntry::Double { .. } => {
                    ans.push(next);
                    ans.push(RuntimeConstantPoolEntry::DummyEntry);
                }
                _ => ans.push(next)
            }
        }
        debug!("CP Done");
        Some(RuntimeConstantPool { constant_pool: ans })
    }
    pub fn new_empty() -> RuntimeConstantPool {
        RuntimeConstantPool {
            constant_pool: vec!()
        }
    }
}

impl Index<usize> for RuntimeConstantPool {
    type Output = RuntimeConstantPoolEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.constant_pool[index - 1]
    }
}

#[derive(Debug)]
/// versions of CPInfo variants, except any symbolic references have been resoved to runtime references
pub enum RuntimeConstantPoolEntry {
    Class(ClassRef),
    Fieldref { class: ClassRef, name: String, type_: ClassRef },
    Methodref { class: ClassRef, name: String, descriptor: String },
    InterfaceMethodref { class: ClassRef, name: String, descriptor: String },
    String(JavaType),
    Integer(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    DummyEntry,
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
    /// the type of the field
    pub class: ClassRef,
    /// the attributes of the field
    pub attributes: Vec<Attribute>,
    /// the value of the field
    pub value: Arc<RwLock<JavaType>>,
}

impl Field {
    pub fn new(class: &JavaClass, field_info: &FieldInfo) -> Field {
        let access_flags = field_info.access_flags;
        let name = jvm::get_name(class, &class.constant_pool[field_info.name_index]);
        let descriptor_raw = jvm::get_name(class, &class.constant_pool[field_info.descriptor_index]);
        let d_r_2 = descriptor_raw.to_owned();
        let mut descriptor_chars = d_r_2.chars();
        let descriptor = parse_type(descriptor_chars.next().unwrap(), &mut descriptor_chars);
        let attributes = field_info.attributes.clone();
        let class = jvm::get_class(&descriptor_raw).unwrap();
        let value = Arc::new(RwLock::new(class.get_default_value()));
        Field { class, access_flags, name, descriptor_raw, descriptor, attributes, value }
    }

    pub fn from_instance_field_info(info: &InstanceFieldInfo) -> Field {
        let value = Arc::new(RwLock::new(info.class.get_default_value()));
        Field {
            access_flags: info.access_flags,
            class: info.class,
            name: info.name.to_owned(),
            descriptor_raw: info.descriptor_raw.to_owned(),
            descriptor: info.descriptor.to_owned(),
            attributes: info.attributes.clone(),
            value,
        }
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
    /// the type of the field
    pub class: ClassRef,
    /// the attributes of the field
    pub attributes: Vec<Attribute>,
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
        let class = jvm::get_class(&descriptor_raw).unwrap();
        InstanceFieldInfo { class, access_flags, name, descriptor_raw, descriptor, attributes }
    }
}

#[derive(Debug)]
pub struct MethodCode {
    pub max_stack: usize,
    pub max_locals: usize,
    pub code: Vec<Opcode>,
    pub exception_table: (), //TODO
    pub code_attrs: (), //TODO
    pub synthetic: bool,
    pub deprecated: bool,
}

impl MethodCode {
    pub fn new(attr: &Attribute) -> Self {
        if let Attribute::Code { max_locals, max_stack, code, exception_table: _, attributes } = attr {
            MethodCode {
                max_stack: *max_stack as usize, max_locals: *max_locals as usize, code: code.clone(),
                exception_table: (), 
                code_attrs: (), 
                synthetic: attributes.iter().any(|a| matches!(a, Attribute::Synthetic)), 
                deprecated: attributes.iter().any(|a| matches!(a, Attribute::Deprecated))
            }
        } else {
            panic!("Attribute passed to MethodCode constructor was not Code")
        }
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
    /// a reference to the class this method is in
    pub class: ClassRef,
    /// a list of the parameters of the method in binary format
    pub parameters: Vec<String>,
    /// the return type of the method in binary format
    pub return_type: String,
    /// the access flags of the methood
    pub access_flags: u16,
    /// the attributes of the function (including the Code attribute)
    pub attributes: Vec<Attribute>,
    /// the extracted code attribute of this method
    /// guaranteed to be Some if this method is not native or abstract
    pub code: Option<MethodCode>,

    pub signature: (), //TODO
    pub visible_annotations: (), //TODO
    pub invisible_annotations: () //TODO
}

impl Method {
    
    pub fn new(class: ClassRef, jc: &JavaClass, method_info: &MethodInfo) -> Option<Method> {
        let (parameters, return_type) =
            parse_parameters_return(&jvm::get_name(jc, &jc.constant_pool[method_info.descriptor_index]));
        let name = jvm::get_name(jc, &jc.constant_pool[method_info.name_index]);
        let descriptor = jvm::get_name(jc, &jc.constant_pool[method_info.descriptor_index]);
        let repr = name.to_owned() + &descriptor;
        let access_flags = method_info.access_flags;
        let attributes = method_info.attributes.clone();
        let code = if method_info.is_abstract() || method_info.is_native() {
            //abstract and native methods have no code attribute
            None
        } else {
            //should never be None, as all concrete methods must have a code attribute
            attributes.iter().find_map(|attr| {
                if let Attribute::Code { .. } = attr { 
                    Some(MethodCode::new(attr)) 
                } else {
                    None
                }
            })
        };
        Some(Method { class, name, descriptor, repr, parameters, return_type, access_flags, attributes, code, signature: (), visible_annotations: (), invisible_annotations: () })
    }
    pub fn is_abstract(&self) -> bool {
        self.access_flags & java_class::methods::AccessFlags::Abstract as u16 != 0
    }
    pub fn is_native(&self) -> bool {
        self.access_flags & java_class::methods::AccessFlags::Native as u16 != 0
    }
    pub fn is_public(&self) -> bool {
        self.access_flags & java_class::methods::AccessFlags::Public as u16 != 0
    }
    pub fn is_protected(&self) -> bool {
        self.access_flags & java_class::methods::AccessFlags::Protected as u16 != 0
    }
    pub fn is_private(&self) -> bool {
        self.access_flags & java_class::methods::AccessFlags::Private as u16 != 0
    }
}

#[derive(Debug)]
pub enum JavaType {
    Boolean(bool),
    Byte(u8),
    Short(i16),
    Char(u16),
    Int(i32),
    Float(f32),
    Long(i64),
    Double(f64),
    Object { class: ClassRef, fields: HashMap<String, JavaType> },
    Array { class: ClassRef, data: Box<[JavaType]> },
    Reference { class: ClassRef, val: Arc<RwLock<JavaType>> },
    Null
}

impl Clone for JavaType {
    fn clone(&self) -> Self {
        match self {
            Byte(val) => Byte(*val),
            Short(val) => Short(*val),
            Char(val) => Char(*val),
            Int(val) => Int(*val),
            Float(val) => Float(*val),
            Long(val) => Long(*val),
            Double(val) => Double(*val),
            Reference {class, val} => Reference {class, val: val.clone()},
            Null => Null,
            _ => panic!("Attempt to clone Object or Array")
        }
    }
}

impl JavaType {
    
    /// Sets an element either of a JavaType::Array or a JavaType::Reference to an array
    pub fn array_get(&self, ind: usize) -> JavaType {
        match self {
            Reference {val, ..} => {
                if let Array {data, ..} = val.read().unwrap().deref() {
                    data[ind].clone()
                } else {
                    panic!()
                }
            },
            Array {data, ..} => {
                data[ind].clone()
            },
            _ => panic!()
        }
    }
    /// Sets an element of an array. Val must be a Reference to an Array.
    pub fn array_set(&self, ind: usize, val: JavaType) {
        if let Reference {val: int, ..} = self {
            int.write().unwrap().array_set_interior(ind, val);
        } else {
            panic!();
        }
    }
    fn array_set_interior(&mut self, ind: usize, val: JavaType) {
        if let Array {data, ..} = self {
            data[ind] = val;
        } else {
            panic!();
        }
    }
    /// Gets the length either of a JavaType::Array or a JavaType::Reference to an array
    pub fn array_length(&self) -> i32 {
        match self {
            Reference { val, ..} => {
                if let Array {data, ..} = val.read().unwrap().deref() {
                    data.len() as i32
                } else {
                    panic!();
                }
            },
            Array {data, ..} => {
                data.len() as i32
            },
            _ => panic!()
        }
    }
    pub fn cast_usize(&self) -> usize {
        match self {
            Byte(val) => *val as usize,
            Short(val) => *val as usize,
            Char(val) => *val as usize,
            Int(val) => *val as usize,
            Float(val) => *val as usize,
            Long(val) => *val as usize,
            Double(val) => *val as usize,
            _ => panic!()
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, JavaType::Null)
    }
    pub fn get_field(&self, name: &str) -> JavaType {
        if let Reference {val, .. } = self {
            if let Object {fields, ..} = val.read().unwrap().deref() {
                fields.get(name).unwrap().clone()
            } else {
                panic!()
            }
        } else {
            panic!()
        }
    }
    pub fn set_field(&self, name: &str, val: JavaType) {
        if let Reference {val: v, ..} = self {
            if let Object {fields, ..} = v.write().unwrap().deref_mut() {
                fields.insert(name.to_owned(), val);
            } else {
                panic!()
            }
        } else {
            panic!()
        }
    }
}

impl ::std::cmp::PartialEq for JavaType {
    fn eq(&self, other: &JavaType) -> bool {
        match self {
            Boolean(val) => match other {
                Boolean(val2) => val == val2,
                _ => false
            }
            Byte(val) => match other {
                Byte(val2) => val == val2,
                _ => false
            },
            Short(val) => match other {
                Short(val2) => val == val2,
                _ => false
            },
            Char(val) => match other {
                Char(val2) => val == val2,
                _ => false
            },
            Int(val) => match other {
                Int(val2) => val == val2,
                _ => false
            },
            Float(val) => match other {
                Float(val2) => val == val2,
                _ => false
            },
            Long(val) => match other {
                Long(val2) => val == val2,
                _ => false
            },
            Double(val) => match other {
                Double(val2) => val == val2,
                _ => false
            },
            Object {..} | Array {..} => false, //should never be called on 2 Objects
            Reference {val: a, ..} => match other {
                Reference {val: b, ..} => Arc::ptr_eq(a, b),
                _ => false
            },
            Null => matches!(other, Null)
        }
    }
}

// pub enum JavaTypeKind {
//     Byte, Char, Short, Int, Long, Float, Double, Ref, 
//     ByteArray, CharArray, ShortArray, IntArray, LongArray,
//     FloatArray, DoubleArray, RefArray
// }

// pub enum SymOpcode {
//     //Stack manipulation
//     Push {type_: JavaTypeKind},
//     Pop {type_: JavaTypeKind},
//     Duplicate {pos: u8, wide: bool},
//     PushImm {type_: JavaTypeKind, val: JavaType},

//     //Local manipulation
//     LoadLocal {index: u16, type_: JavaTypeKind},
//     StoreLocal {index: u16, type_: JavaTypeKind},

//     //Arrays
//     LoadFromArray {type_: JavaTypeKind},
//     StoreToArray {type_: JavaTypeKind},
//     GetArrayLength,
//     NewPrimitiveArray {type_: JavaTypeKind},
//     NewRefArray {type_: Arc<RwLock<JavaClass>>},
//     NewMultiArray {array_type: Arc<RwLock<JavaClass>>, dimensions: u8},
    
//     //Control flow
//     Return {type_: JavaTypeKind},
//     Throw,
//     Breakpoint,

//     //Types
//     CheckCast {type_: Arc<RwLock<JavaClass>>},
//     Cast {from: JavaTypeKind, to: JavaTypeKind},

//     //Primitive operations
//     Add {type_: JavaTypeKind},
//     Subtract {type_: JavaTypeKind},
//     Multiply {type_: JavaTypeKind},
//     Divide {type_: JavaTypeKind},
//     Negate {type_: JavaTypeKind},
//     Remainder {type_: JavaTypeKind},
//     Compare {type_: JavaTypeKind},
//     CompareNaNIsLess {type_: JavaTypeKind},

//     //Reference operations
//     GetField {class: Arc<RwLock<JavaClass>>, name: String},
//     GetStatic {class: Arc<RwLock<JavaClass>>, name: String}
// }