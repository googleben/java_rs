#![feature(c_variadic, vec_into_raw_parts, arbitrary_self_types)]

use std::env;

use env_logger::Env;

extern crate env_logger;
/// # Explanation of "binary" names:
/// In the JVM, a "binary" name is a type's name as it appears in a .class file.
/// Primitives are represented as one character (int=I, long=J)
/// Non-array objects are represented by their complete package path and name using '/' as a separator:
/// java.lang.System = java/lang/System, com.site.subdomain.package.ClassName = com/site/subdomain/package/ClassName
/// When used in context with other types (i.e. in descriptors of methods) they are prefixed by 'L' and ended with ';'
/// In exposed methods once the JVM is started, all object descriptors should not have 'L'-';'
/// Arrays are represented by '[' followed by the type:
/// int[] = [I, long[][] = [[L, Object[] = [java/lang/Object
/// Before being processed, arrays have 'L'-';' when describing an array of a non-array Object type


extern crate java_class;
#[macro_use]
extern crate log;
extern crate zip;
extern crate va_list;

//will uncomment the following when it's time to work on the JNI
//pub mod jni;
//pub mod jni_impl;
//TODO: add the jni extension "jvm.h"
pub mod types;
pub mod jvm;
pub mod threads;
pub mod jni;
pub mod jni_impl;

fn main() {
    //env_logger::init();
    ::env::set_var("RUST_BACKTRACE", "1");
    env_logger::Builder::from_env(Env::default().default_filter_or("trace")).init();
    let cp = Box::new([".".to_owned()]);
    jvm::start(cp, &"Tester".to_owned());
    //println!("{:?}", jvm::load_class(&"Tester".to_owned()));
    match jvm::load_class(&"Tester".to_owned()) {
        Some(_) => println!("ok"),
        None => println!("err")
    }
}