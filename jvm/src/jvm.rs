use java_class::class::JavaClass;
use java_class::cp_info::CPInfo;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::sync::{Arc, RwLock};
use types;
use types::{Class, ClassRef};
use types::JavaType;
use zip::ZipArchive;
use threads::*;

static mut JVM_INSTANCE: *const Arc<RwLock<JVM>> = 0 as *const Arc<RwLock<JVM>>;

const MAJOR_VERSION: u16 = 52;
const MINOR_VERSION: u16 = 0;

/// returns a "safe" reference to the static JVM
fn jvm() -> Arc<RwLock<JVM>> {
    unsafe {
        (*JVM_INSTANCE).clone()
    }
}

/// struct containing all runtime information about the JVM
struct JVM {
    pub jars: Vec<ZipArchive<File>>,
    pub classpath: Box<[String]>,
    pub classes: HashMap<String, ClassRef>,
    pub to_load: Vec<String>,
    pub to_init: Vec<(ClassRef, Arc<Box<JavaClass>>)>,
    pub objects: Vec<Arc<RwLock<JavaType>>>,
    pub interned_strings: Vec<(String, JavaType)>,
    ///true if a class is currently being initialized
    pub is_in_init_loop: bool
}

/// starts the JVM
/// # parametersClass
/// * classpath: a list of folders or jar files to search for runtime classes
/// * entry: the class containing the main function/entry point to execute
pub fn start(classpath: Box<[String]>, entry_point: &str) {
    info!("Starting JVM");
    debug!("Start");
    let mut jars = vec!();
    let java_path = r#"C:\Program Files\Java\jdk1.8.0_201"#;
    jars.push(ZipArchive::new(File::open(java_path.to_owned() + "/jre/lib/rt.jar").unwrap()).unwrap());
    jars.push(ZipArchive::new(File::open(java_path.to_owned() + "/jre/lib/jce.jar").unwrap()).unwrap());
    // jars.push(ZipArchive::new(File::open(::std::env::var("JAVA_HOME").unwrap() + "/jre/lib/rt.jar").unwrap()).unwrap());
    // jars.push(ZipArchive::new(File::open(::std::env::var("JAVA_HOME").unwrap() + "/jre/lib/jce.jar").unwrap()).unwrap());
    //let stdlib = ZipArchive::new(File::open(::std::env::var("JAVA_HOME").unwrap()+"/jre/lib/rt.jar").unwrap()).unwrap();
    let jvm = JVM {
        jars,
        classpath,
        classes: HashMap::<String, ClassRef>::new(),
        to_load: Vec::new(),
        to_init: Vec::new(),
        objects: Vec::new(),
        interned_strings: Vec::new(),
        is_in_init_loop: false
    };
    unsafe {
        JVM_INSTANCE = ::std::mem::transmute(Box::new(Arc::new(RwLock::new(jvm))));
    }
    if let Some(entry_class) = load_class(entry_point) {
        if let Some(main) = entry_class.methods.get("main:([:java/lang/String;)V") {
            let args = vec![JavaType::Null];
            let _main_thread = JvmThread::with_args(main, args);
        }
    }
}

pub fn create_array(member_class: ClassRef, len: usize) -> JavaType {
    let member_class_name = &member_class.name;
    let class_name = "[".to_owned()+member_class_name;
    let class = get_or_load_class(&class_name).unwrap();
    let data = vec![member_class.get_default_value(); len].into_boxed_slice();
    let arr = JavaType::Array {class, data};
    let val = Arc::new(RwLock::new(arr));
    add_to_gc(val.clone());
    JavaType::Reference {class, val}
}

/// returns the reference to a class if it has been loaded
pub fn get_class(name: &str) -> Option<ClassRef> {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    match jvm.classes.get(name) {
        Some(arc) => Some(arc),
        None => None
    }
}

fn find_class_jar(name: &str) -> Option<Vec<u8>> {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    for jar in &mut jvm.jars {
        let x = jar.by_name(&(name.to_string()+".class"));
        let mut file = match x {
            Ok(s) => s,
            Err(_) => continue
        };
        let mut ans = vec!();
        use std::io::Read;
        if let Err(a) = file.read_to_end(&mut ans) {
            error!("Error reading from jar: {:?}", a);
            return None;
        }
        return Some(ans)
    }
    None
}

/// returns the path to a .class file of a given class, if it exists and is in the classpath
fn find_class(name: &str) -> Option<PathBuf> {
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
pub fn is_class_loaded(name: &str) -> bool {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    jvm.classes.contains_key(name)
}

/// If the class is defined, return it, otherwise attempt to load it
pub fn get_or_load_class(name: &str) -> Option<ClassRef> {
    //if the class is already defined return it
    if is_class_loaded(name) {
        return Some(get_class(name).unwrap()); //OK to unwrap since is_class_loaded guarantees existance
    }
    //otherwise, attempt to load it
    load_class(name)
}

pub fn add_to_load(name: &str) {
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

fn get_to_init() -> (ClassRef, Arc<Box<JavaClass>>) {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    let i = jvm.to_init.len() - 1;
    jvm.to_init.remove(i)
}

fn is_in_init_loop() -> bool {
    let jvm = jvm();
    let jvm = jvm.read().unwrap();
    jvm.is_in_init_loop
}

fn set_in_init_loop(val: bool) {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    jvm.is_in_init_loop = val;
}

fn load_while() -> Option<()> {
    while has_to_load() {
        let to_l = get_to_load();
        if is_class_loaded(&to_l) {
            continue;
        }
        debug!("Loading class {}", to_l);
        load_class_2(&to_l)?;
        debug!("Done loading class {}", to_l);
    }
    Some(())
}

//TODO: On error return Err(Throwable) JVM Specification §5.3.5
//TODO: Completely separate classloading to loading and intializing
//TODO: Pause all active threads if loading a class after initial startup
pub fn load_class(name: &str) -> Option<ClassRef> {
    let ans = load_class_2(name);
    if is_in_init_loop() {
        trace!("Init loop already running, returning early from load_class");
        return ans;
    }
    if has_to_init() || has_to_load() {
        debug!("Beginning initialization loop after loading {}", name);
        set_in_init_loop(true);
    } else {
        debug!("Loaded {} and there are no classes to initialize", name);
    }
    

    while has_to_init() || has_to_load() {
        load_while()?;
        if has_to_init() {
            debug!("Found class to initialize");
            let cx = get_to_init();

            let name_2 = if cx.1.constant_pool.len() > 1 {
                get_name_cp(&cx.1.constant_pool, cx.1.this_class)
            } else {
                "Unknown class name".to_string()
            };
            debug!("Initializing {}", name_2);
            unsafe {
                let c = std::mem::transmute::<ClassRef, *mut Class>(cx.0);
                (&mut *c).initialize(&cx.1, cx.0)?;
            }
            
            debug!("Done initializing {}", name_2);
        }
    }
    set_in_init_loop(false);
    debug!("No more classes to initialize");
    ans
}

fn add_to_init(class: ClassRef, jc: Box<JavaClass>) {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    jvm.to_init.push((class, Arc::new(jc)));
}

/// Load a class using the bootstrap classloader
fn load_class_2(name: &str) -> Option<ClassRef> {
    //if the class is a primitive or an array, special case load

    let mut chars = name.chars();
    let c = chars.next().unwrap();
    match c {
        'B' | 'C' | 'D' | 'F' |
        'I' | 'J' | 'S' | 'Z' => {
            // let name = match c {
            //     'B' => "byte",
            //     'C' => "char",
            //     'D' => "double",
            //     'F' => "float",
            //     'I' => "int",
            //     'J' => "long",
            //     'S' => "short",
            //     'Z' => "boolean",
            //     _ => panic!() //unreachable
            // }.to_owned();
            let name = format!("{}", c);
            let class = Class {
                major_version: MAJOR_VERSION,
                minor_version: MINOR_VERSION,
                constant_pool: types::RuntimeConstantPool::new_empty(),
                access_flags: 0,
                name,
                super_class: None,
                interfaces: vec!(),
                fields: HashMap::new(),
                instance_fields: vec!(),
                methods: HashMap::new(),
                attributes: vec!(),
                array_inner: None
            };
            let jvm = jvm();
            let mut jvm = jvm.write().unwrap();
            let class: ClassRef = Box::leak(Box::new(class));
            jvm.classes.insert(format!("{}", c), class);
            return Some(class);
        }
        '[' => {
            let mut sub_name = chars.as_str().to_string();
            if &sub_name[..1] == "L" {
                let x = sub_name[1..sub_name.len() - 1].to_owned();
                sub_name = x;
            }
            let class = Class {
                major_version: MAJOR_VERSION,
                minor_version: MINOR_VERSION,
                constant_pool: types::RuntimeConstantPool::new_empty(),
                access_flags: 0,
                name: name.to_owned(),
                super_class: Some(get_or_load_class(&"java/lang/Object".to_string())?),
                interfaces: vec!(),
                fields: HashMap::new(),
                instance_fields: vec!(),
                methods: HashMap::new(),
                attributes: vec!(),
                array_inner: None
            };
            let jvm = jvm();
            let mut jvm = jvm.write().unwrap();
            let class: ClassRef = Box::leak(Box::new(class));
            jvm.classes.insert(name.to_string(), class);
            drop(jvm);
            if &sub_name[..1] == "[" {
                load_class_2(&sub_name)?;
            } else {
                add_to_load(&sub_name);
            }
            add_to_init(class, Box::new(JavaClass::empty()));
            return Some(class);
        }
        _ => {}
    };
    //resolve the path of the .class file

    //load the .class into a static representation
    debug!("Trying to load class {}", name);
    let jc = match find_class_jar(&name) {
        Some(bytes) => {
            match JavaClass::new_from_bytes(bytes) {
                Ok(c) => c,
                Err(a) => {
                    error!("Class {} could not be loaded from zip: {:?}", name, a);
                    return None;
                }
            }
        }
        None => {
            match find_class(&name) {
                Some(p) => match JavaClass::new(p.to_str().unwrap()) {
                    Ok(c) => c,
                    Err(_) => return None
                },
                None => {
                    error!("Class could not be found");
                    panic!();
                    return None;
                }
            }
        }
    };
    let jc = Box::new(jc);
    debug!("Loaded .class file");
    //load superinterfaces and superclasses
    //if the class is not java.lang.Object, attempt to load its superclass
    if name != "java/lang/Object" {
        let super_class_index = jc.super_class;
        let super_class_name = get_name(&jc, &jc.constant_pool[super_class_index]);
        //a class may not be its own superclass
        if super_class_name==name {
            return None;
        }
        //superclasses may not be interfaces
    }
    for interface_index in &jc.interfaces {
        let interface_name = get_name(&jc, &jc.constant_pool[*interface_index]);
        //an interface may not be its own superinterface
        if interface_name==name {
            return None;
        }
    }
    debug!("Making Class struct");
    let class = Class::default();
    class.initialize_start(&jc)?;
    
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    let class: ClassRef = Box::leak(Box::new(class));
    jvm.classes.insert(name.to_string(), class);
    drop(jvm);
    add_to_init(class, jc);
    Some(class)
}

/// Registers an object with the garbage collector
pub fn add_to_gc(obj: Arc<RwLock<JavaType>>) {
    let jvm = jvm();
    let mut jvm = jvm.write().unwrap();
    jvm.objects.push(obj);
}

pub fn get_or_intern_string(str: String) -> JavaType {
    {
        let jvm = jvm();
        let jvm = jvm.read().unwrap();
        for (s, ans) in &jvm.interned_strings {
            if s == &str {
                return ans.clone();
            }
        }
    }
    let string_class = get_or_load_class("java/lang/String").unwrap();
    let obj = string_class.instantiate_no_gc();
    let encoded: Vec<u16> = str.encode_utf16().collect();
    let char_class = get_or_load_class("C").unwrap();
    let arr = create_array(char_class, encoded.len());
    for (i, &c) in encoded.iter().enumerate() {
        arr.array_set(i, JavaType::Char(c));
    }
    obj.set_field("value", arr);
    obj
}