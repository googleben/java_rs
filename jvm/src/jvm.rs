use std::sync::{Arc, Mutex};
use java_class::cp_info::CPInfo;
use types::JavaType;
use std::path::PathBuf;
use std::path::Path;
use std::str;
use types::Class;
use std::collections::HashMap;
use java_class::class::JavaClass;

static mut JVM_INSTANCE: *const Arc<Mutex<JVM>> = 0 as *const Arc<Mutex<JVM>>;

/// returns a "safe" reference to the static JVM
fn jvm<'a>() -> Arc<Mutex<JVM>> {
    unsafe {
        (*JVM_INSTANCE).clone()
    }
}

/// struct containing all runtime information about the JVM
struct JVM {
    pub classpath: Box<[String]>,
    pub classes: HashMap<String, Arc<Mutex<Class>>>
}

/// starts the JVM
/// # parametersClass
/// * classpath: a list of folders or jar files to search for runtime classes
/// * entry: the class containing the main function/entry point to execute
pub fn start(classpath: Box<[String]>, entry_point: &String) {
    let jvm = JVM {
        classpath,
        classes: HashMap::<String, Arc<Mutex<Class>>>::new()
    };
    unsafe {
        JVM_INSTANCE = ::std::mem::transmute(Box::new(Arc::new(Mutex::new(jvm))));
    }
}

/// returns the reference to a class if it has been loaded
pub fn get_class(name: &String) -> Option<Arc<Mutex<Class>>> {
    let jvm = jvm();
    let jvm = jvm.lock().unwrap();
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
    let jvm = jvm.lock().unwrap();
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
    let jvm = jvm.lock().unwrap();
    jvm.classes.contains_key(name)
}

//TODO: Pass up throwable from load_class
/// If the class is defined, return it, otherwise attempt to load it
pub fn get_or_load_class(name: &String) -> Result<Arc<Mutex<Class>>, ()> {
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

//TODO: On error return Err(Throwable) §5.3.5
/// Load a class using the bootstrap classloader
pub fn load_class(name: &String) -> Result<Arc<Mutex<Class>>, ()> {
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
        let super_class = match get_or_load_class(&super_class_name) {
            Ok(a) => a,
            Err(a) => return Err(a)
        };
        let super_class = super_class.lock().unwrap();
        //superclasses may not be interfaces
        if super_class.class.is_interface()  {
            return Err(());
        }
    }
    for interface_index in &class.interfaces {
        let interface_name = get_name(&class, &class.constant_pool[*interface_index]);
        //an interface may not be its own superinterface
        if &interface_name==name {
            return Err(());
        }
        let interface = match get_or_load_class(&interface_name) {
            Ok(a) => a,
            Err(a) => return Err(a)
        };
        let interface = interface.lock().unwrap();
        //must be an interface
        if !interface.class.is_interface() {
            return Err(());
        }
    }
    let jc = Class::new(class);
    let arc = Arc::new(Mutex::new(jc));
    let jvm = jvm();
    let mut jvm = jvm.lock().unwrap();
    jvm.classes.insert(name.to_string(), arc.clone());
    Ok(arc)
}