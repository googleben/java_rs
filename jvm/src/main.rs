extern crate java_class;
//will uncomment the following when it's time to work on the JNI
//pub mod jni;
//pub mod jni_impl;
//TODO: add the jni extension "jvm.h"
pub mod types;
pub mod jvm;

fn main() {
    let cp = Box::new(["E:\\".to_owned()]);
    jvm::start(cp, &"Tester".to_owned());
    println!("{:?}", jvm::load_class(&"Tester".to_owned()));
}