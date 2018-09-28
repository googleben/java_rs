use java_class::class;
use java_class::class::JavaClass;
use java_class::cp_info::CPInfo;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::{Arc, RwLock};
use types;
use types::Class;
use types::JavaType;
use zip::read::ZipFile;
use zip::ZipArchive;

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
    pub jars: Vec<ZipArchive<File>>,
    pub classpath: Box<[String]>,
    pub classes: HashMap<String, Arc<RwLock<Class>>>,
    pub to_load: Vec<String>,
    pub to_init: Vec<(Arc<RwLock<Class>>, Arc<Box<JavaClass>>)>,
}

/// starts the JVM
/// # parametersClass
/// * classpath: a list of folders or jar files to search for runtime classes
/// * entry: the class containing the main function/entry point to execute
pub fn start(classpath: Box<[String]>, entry_point: &String) {
    info!("Starting JVM");
    debug!("Start");
    let mut jars = vec!();
    jars.push(ZipArchive::new(File::open(::std::env::var("JAVA_HOME").unwrap() + "/jre/lib/rt.jar").unwrap()).unwrap());
    jars.push(ZipArchive::new(File::open(::std::env::var("JAVA_HOME").unwrap() + "/jre/lib/jce.jar").unwrap()).unwrap());
    //let stdlib = ZipArchive::new(File::open(::std::env::var("JAVA_HOME").unwrap()+"/jre/lib/rt.jar").unwrap()).unwrap();
    let jvm = JVM {
        jars,
        classpath,
        classes: HashMap::<String, Arc<RwLock<Class>>>::new(),
        to_load: Vec::new(),
        to_init: Vec::new(),
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

fn find_class_jar(name: &String) -> Option<Vec<u8>> {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    for mut jar in &mut jvm.jars {
        let x = jar.by_name(&(name.to_string() + ".class"));
        let mut file = match x {
            Ok(s) => s,
            Err(_) => continue
        };
        let mut ans = vec!();
        use std::io::Read;
        match file.read_to_end(&mut ans) {
            Err(a) => {
                error!("Error reading from jar: {:?}", a);
                return None;
            }
            Ok(_) => {}
        };
        return Some(ans);
    }
    None
}

/// returns the path to a .class file of a given class, if it exists and is in the classpath
fn find_class(name: &String) -> Option<PathBuf> {
    let mut parts = Vec::new();
    name.split('/').for_each(|s| parts.push(s.to_owned()));
    let l = parts.len();
    parts[l - 1] += ".class";
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
    error!("Could not find class {}", name);
    None
}

/// returns true if the bootstrap classloader has already loaded the class
pub fn is_class_loaded(name: &String) -> bool {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    jvm.classes.contains_key(name)
}

/// If the class is defined, return it, otherwise attempt to load it
pub fn get_or_load_class(name: &String) -> Result<Arc<RwLock<Class>>, ()> {
    //if the class is already defined return it
    if is_class_loaded(name) {
        return Ok(get_class(name).unwrap()); //OK to unwrap since is_class_loaded guarantees existance
    }
    //otherwise, attempt to load it
    load_class(name)
}

pub fn add_to_load(name: &String) {
    if !is_class_loaded(name) {
        let jvm = jvm();
        let mut jvm = jvm.write().unwrap();
        jvm.to_load.push(name.to_owned());
    }
}

/// helper function to get the names of values from a "wrapper" CPInfo struct such as CPInfo::Class
pub fn get_name(class: &JavaClass, info: &CPInfo) -> String {
    match info {
        CPInfo::Class { name_index } => {
            get_name(class, &class.constant_pool[*name_index])
        }
        CPInfo::Utf8 { bytes, .. } => {
            ::java_class::class::read_string(bytes)
        }
        _ => panic!("Invalid CPInfo for get_name")
    }
}

pub fn get_name_cp(cp: &::java_class::cp::ConstantPool, index: u16) -> String {
    match &cp[index] {
        CPInfo::Class { name_index } => {
            get_name_cp(cp, *name_index)
        }
        CPInfo::String { string_index } => {
            get_name_cp(cp, *string_index)
        }
        CPInfo::Utf8 { bytes, .. } => {
            ::java_class::class::read_string(bytes)
        }
        _ => panic!("Invalid CPInfo for get_name")
    }
}

fn has_to_load() -> bool {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    !jvm.to_load.is_empty()
}

fn has_to_init() -> bool {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    !jvm.to_init.is_empty()
}

fn get_to_load() -> String {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    jvm.to_load.remove(0)
}

fn get_to_init() -> (Arc<RwLock<Class>>, Arc<Box<JavaClass>>) {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    let i = jvm.to_init.len() - 1;
    jvm.to_init.remove(i)
}

fn load_while() -> Result<(), ()> {
    while has_to_load() {
        let to_l = get_to_load();
        if is_class_loaded(&to_l) {
            continue;
        }
        debug!("Loading class {}", to_l);
        load_class_2(&to_l)?;
        debug!("Done loading class {}", to_l);
    }
    Ok(())
}

//TODO: On error return Err(Throwable) JVM Specification §5.3.5
pub fn load_class(name: &String) -> Result<Arc<RwLock<Class>>, ()> {
    let ans = load_class_2(name);
    debug!("Loaded {}, intializing stuff", name);

    while has_to_init() || has_to_load() {
        load_while()?;
        if has_to_init() {
            debug!("Found class to initialize");
            let cx = get_to_init();
            let mut c = cx.0.write().unwrap();
            let mut name_2 = "Unknown class name".to_string();
            if cx.1.constant_pool.len() > 1 {
                name_2 = get_name_cp(&cx.1.constant_pool, cx.1.this_class);
            }
            debug!("Initializing {}", name_2);
            c.initialize(&cx.1)?;
            debug!("Done initializing {}", name_2);
        }
    }
    debug!("No more classes to initialize");
    ans
}

fn add_to_init(class: Arc<RwLock<Class>>, jc: Box<JavaClass>) {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    jvm.to_init.push((class, Arc::new(jc)));
}

/// Load a class using the bootstrap classloader
fn load_class_2(name: &String) -> Result<Arc<RwLock<Class>>, ()> {
    //if the class is a primitive or an array, special case load

    let mut chars = name.chars();
    let c = chars.next().unwrap();
    match c {
        'B' | 'C' | 'D' | 'F' |
        'I' | 'J' | 'S' | 'Z' => {
            let name = match c {
                'B' => "byte",
                'C' => "char",
                'D' => "double",
                'F' => "float",
                'I' => "int",
                'J' => "long",
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
                attributes: vec!(),
            }));
            let jvm = jvm();
            let mut jvm = jvm.write().unwrap();
            jvm.classes.insert(name.to_owned(), class.clone());
            return Ok(class);
        }
        '[' => {
            let mut sub_name = chars.as_str().to_string();
            if &sub_name[..1] == "L" {
                let x = sub_name[1..sub_name.len() - 1].to_owned();
                sub_name = x;
            }
            let class = Arc::new(RwLock::new(Class {
                major_version: MAJOR_VERSION,
                minor_version: MINOR_VERSION,
                constant_pool: types::SymbolicConstantPool::new_empty(),
                access_flags: 0,
                name: name.to_owned(),
                super_class: Some(get_or_load_class(&"java/lang/Object".to_string())?),
                interfaces: vec!(),
                fields: HashMap::new(),
                instance_fields: vec!(),
                methods: HashMap::new(),
                attributes: vec!(),
            }));
            let jvm = jvm();
            let mut jvm = jvm.write().unwrap();
            jvm.classes.insert(name.to_owned(), class.clone());
            drop(jvm);
            if &sub_name[..1] == "[" {
                load_class_2(&sub_name)?;
            } else {
                add_to_load(&sub_name);
            }
            add_to_init(class.clone(), Box::new(JavaClass::empty()));
            return Ok(class);
        }
        _ => {}
    };
    //resolve the path of the .class file

    //load the .class into a static representation
    debug!("Trying to load class {}", name);
    let class = match find_class_jar(&name) {
        Some(bytes) => {
            match JavaClass::new_from_bytes(bytes) {
                Ok(c) => c,
                Err(a) => {
                    error!("Class could not be loaded from zip: {:?}", a);
                    return Err(());
                }
            }
        }
        None => {
            match find_class(&name) {
                Some(p) => match JavaClass::new(p.to_str().unwrap()) {
                    Ok(c) => c,
                    Err(_) => return Err(())
                },
                None => {
                    error!("Class could not be found");
                    return Err(());
                }
            }
        }
    };
    let class = Box::new(class);
    debug!("Loaded .class file");
    //load superinterfaces and superclasses
    //if the class is not java.lang.Object, attempt to load its superclass
    if name != "java/lang/Object" {
        let super_class_index = class.super_class;
        let super_class_name = get_name(&class, &class.constant_pool[super_class_index]);
        //a class may not be its own superclass
        if &super_class_name == name {
            return Err(());
        }
        //superclasses may not be interfaces
    }
    for interface_index in &class.interfaces {
        let interface_name = get_name(&class, &class.constant_pool[*interface_index]);
        //an interface may not be its own superinterface
        if &interface_name == name {
            return Err(());
        }
    }
    debug!("Making Class struct");
    let jc = Class::new();
    jc.initialize_start(&class)?;
    let arc = Arc::new(RwLock::new(jc));
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    jvm.classes.insert(name.to_string(), arc.clone());
    drop(jvm);
    let c = arc.clone();
    add_to_init(c, class);
    Ok(arc)
}