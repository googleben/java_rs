use std::sync::{Arc, RwLock};
use java_class::cp_info::CPInfo;
use types::JavaType;
use types;
use std::path::PathBuf;
use std::path::Path;
use std::str;
use types::Class;
use std::collections::HashMap;
use java_class::class::JavaClass;
use java_class::class;

static mut JVM_INSTANCE: *const Arc<RwLock<JVM>> = 0 as *const Arc<RwLock<JVM>>;

const MAJOR_VERSION: u16 = 52;
const MINOR_VERSION: u16 = 0;

/// returns a "safe" reference to the static JVM
fn jvm<'a>() -> Arc<RwLock<JVM>> {
    unsafe {
        (*JVM_INSTANCE).clone()
    }
}

/// struct containing all runtime information about the JVM
struct JVM {
    pub classpath: Box<[String]>,
    pub classes: HashMap<String, Arc<RwLock<Class>>>
}

/// starts the JVM
/// # parametersClass
/// * classpath: a list of folders or jar files to search for runtime classes
/// * entry: the class containing the main function/entry point to execute
pub fn start(classpath: Box<[String]>, entry_point: &String) {
    let jvm = JVM {
        classpath,
        classes: HashMap::<String, Arc<RwLock<Class>>>::new()
    };
    unsafe {
        JVM_INSTANCE = ::std::mem::transmute(Box::new(Arc::new(RwLock::new(jvm))));
    }
}

/// returns the reference to a class if it has been loaded
pub fn get_class(name: &String) -> Option<Arc<RwLock<Class>>> {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    match jvm.classes.get(name) {
        Some(arc) => Some(arc.clone()),
        None => None
    }
}

/// returns the path to a .class file of a given class, if it exists and is in the classpath
fn find_class(name: &String) -> Option<PathBuf> {
    let mut parts = Vec::new();
    name.split('/').for_each(|s| parts.push(s.to_owned()));
    let l = parts.len();
    parts[l-1] += ".class";
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    for bpath in jvm.classpath.iter() {
        let mut p = Path::new(bpath).to_path_buf();
        for x in &parts {
            p = p.join(x);
        }
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// returns true if the bootstrap classloader has already loaded the class
pub fn is_class_loaded(name: &String) -> bool {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    jvm.classes.contains_key(name)
}

//TODO: Pass up throwable from load_class
/// If the class is defined, return it, otherwise attempt to load it
pub fn get_or_load_class(name: &String) -> Result<Arc<RwLock<Class>>, ()> {
    //if the class is already defined return it
    if is_class_loaded(name) {
        return Ok(get_class(name).unwrap()); //OK to unwrap since is_class_loaded guarantees existance
    }
    //otherwise, attempt to load it
    load_class(name)
}

/// helper function to get the names of values from a "wrapper" CPInfo struct such as CPInfo::Class
pub fn get_name(class: &JavaClass, info: &CPInfo) -> String {
    match info {
        CPInfo::Class {name_index} => {
            get_name(class, &class.constant_pool[*name_index])
        },
        CPInfo::Utf8 {bytes, ..} => {
            str::from_utf8(&bytes).unwrap().to_owned()
        },
        _ => panic!("Invalid CPInfo for get_name")
    }
}

pub fn get_name_cp(cp: &::java_class::cp::ConstantPool, index: u16) -> String {
    match &cp[index] {
        CPInfo::Class {name_index} => {
            get_name_cp(cp, *name_index)
        },
        CPInfo::String {string_index} => {
            get_name_cp(cp, *string_index)
        },
        CPInfo::Utf8 {bytes, ..} => {
            str::from_utf8(&bytes).unwrap().to_owned()
        },
        _ => panic!("Invalid CPInfo for get_name")
    }
}

//TODO: On error return Err(Throwable) §5.3.5
/// Load a class using the bootstrap classloader
pub fn load_class(name: &String) -> Result<Arc<RwLock<Class>>, ()> {
    //if the class is a primitive or an array, special case load
    let mut chars = name.chars();
    let c = chars.next().unwrap();
    match c {
        'B' | 'C' | 'D' | 'F' |
        'I' | 'S' | 'Z' => {
            let name = match c {
                'B' => "byte",
                'C' => "char",
                'D' => "double",
                'F' => "float",
                'I' => "int",
                'S' => "short",
                'Z' => "boolean",
                _ => panic!() //unreachable
            }.to_owned();
            let class = Arc::new(RwLock::new(Class {
                major_version: MAJOR_VERSION,
                minor_version: MINOR_VERSION,
                constant_pool: types::SymbolicConstantPool::new_empty(),
                access_flags: 0,
                name: name.to_owned(),
                super_class: None,
                interfaces: vec!(),
                fields: HashMap::new(),
                instance_fields: vec!(),
                methods: HashMap::new(),
                attributes: vec!()
            }));
            let jvm = jvm();
            let mut jvm = jvm.write().unwrap();
            jvm.classes.insert(name.to_owned(), class.clone());
            return Ok(class);
        }
        '[' => {
            let subclass = load_class(&chars.as_str().to_string())?;
            let access_flags = subclass.read().unwrap().access_flags & class::AccessFlags::Public as u16;
            let sub_name = &subclass.read().unwrap().name;
            let class = Arc::new(RwLock::new(Class {
                major_version: MAJOR_VERSION,
                minor_version: MINOR_VERSION,
                constant_pool: types::SymbolicConstantPool::new_empty(),
                access_flags,
                name: name.to_owned(),
                super_class: Some(get_or_load_class(&"java/lang/Object".to_string())?),
                interfaces: vec!(),
                fields: HashMap::new(),
                instance_fields: vec!(),
                methods: HashMap::new(),
                attributes: vec!()
            }));
            let jvm = jvm();
            let mut jvm = jvm.write().unwrap();
            jvm.classes.insert(name.to_owned(), class.clone());
            return Ok(class);
        }
        _ => {}
    };
    //resolve the path of the .class file
    let path = match find_class(&name) {
        Some(p) => p,
        None => return Err(())
    };
    //load the .class into a static representation
    let class = match JavaClass::new(path.to_str().unwrap()) {
        Ok(c) => c,
        Err(_) => return Err(())
    };
    //load superinterfaces and superclasses
    //if the class is not java.lang.Object, attempt to load its superclass
    if name!="java/lang/Object" {
        let super_class_index = class.super_class;
        let super_class_name = get_name(&class, &class.constant_pool[super_class_index]);
        //a class may not be its own superclass
        if &super_class_name==name {
            return Err(());
        }
        let super_class = get_or_load_class(&super_class_name)?;
        let super_class = super_class.read().unwrap();
        //superclasses may not be interfaces
        if super_class.is_interface()  {
            return Err(());
        }
    }
    for interface_index in &class.interfaces {
        let interface_name = get_name(&class, &class.constant_pool[*interface_index]);
        //an interface may not be its own superinterface
        if &interface_name==name {
            return Err(());
        }
        let interface = get_or_load_class(&interface_name)?;
        let interface = interface.read().unwrap();
        //must be an interface
        if !interface.is_interface() {
            return Err(());
        }
    }
    let jc = Class::new(class);
    let arc = Arc::new(RwLock::new(jc?));
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    jvm.classes.insert(name.to_string(), arc.clone());
    Ok(arc)
}