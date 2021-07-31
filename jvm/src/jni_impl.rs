#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, unused_variables, dead_code, clippy::missing_safety_doc)]

use std::{ffi::{CStr, CString, VaListImpl}, mem, sync::{Arc, RwLock, Weak}};

use java_class::class::JavaClass;
use jni::*;

use crate::{jvm::get_or_intern_string, threads::{JvmThread, ensure_class_init}, types::{Class, ClassRef, Field, InstanceFieldInfo, JavaType, Method, Unwrap}};

macro_rules! jni_exception {
    ($ex_type:expr) => {
        println!("{} exception", $ex_type);
    };
}

pub enum JniRef {
    WeakGlobal(&'static Class, Weak<RwLock<JavaType>>),
    Global(JavaType),
    Local(JavaType)
}

impl JniRef {
    pub fn new_local(val: JavaType) -> *mut JniRef {
        Box::into_raw(Box::new(JniRef::Local(val)))
    }
    pub fn new_global(val: JavaType) -> *mut JniRef {
        Box::into_raw(Box::new(JniRef::Global(val)))
    }
    pub fn new_weak_global(val: JavaType) -> *mut JniRef {
        if let JavaType::Reference {class, val} = val {
            Box::into_raw(Box::new(JniRef::WeakGlobal(class, Arc::downgrade(&val))))
        } else {
            panic!()
        }
    }
    pub unsafe fn delete(val: *mut JniRef) {
        Box::from_raw(val);
    }
    pub unsafe fn get_ref(self: *mut JniRef) -> JavaType {
        match &*self {
            JniRef::Local(val) => val.clone(),
            JniRef::Global(val) => val.clone(),
            JniRef::WeakGlobal(c, r) => {
                if let Some(val) = r.upgrade() {
                    JavaType::Reference {class: c, val}
                } else {
                    JavaType::Null
                }
            }
        }
    }
    pub unsafe fn get_class(self: *mut JniRef) -> &'static Class {
        match &*self {
            JniRef::Local(val) | JniRef::Global(val) => {
                if let JavaType::Reference {class, ..} = val {
                    class
                } else {
                    panic!()
                }
            },
            JniRef::WeakGlobal(c, r) => {
                c
            }
        }
    }
}

const NULL: jobject = 0 as jobject;

fn to_jobject(val: JavaType) -> jobject {
    if let JavaType::Reference {val, ..} = val {
        todo!()
    } else {
        panic!();
    }
}

fn get_thread(env: *mut JNIEnv) -> *mut JvmThread {
    env as *mut JvmThread
}

fn create_local(env: *mut JNIEnv, val: JavaType) -> jobject {
    let thread = get_thread(env);
    unsafe {thread.as_mut()}.unwrap().create_jni_local(val) as jobject
}

unsafe fn get_class_from_jclass(jclass: jclass) -> ClassRef {
    let class = (jclass as *mut JniRef).get_ref().get_field("class");
    if let JavaType::Object {class, ..} = class {
        class
    } else {
        panic!()
    }
}

unsafe fn from_cstr(cstr: *const ::std::os::raw::c_char) -> &'static str {
    let s = CStr::from_ptr(cstr);
    s.to_str().unwrap()
}

enum JavaReturnType {
    Object(jobject),
    Byte(i8),
    Char(u16),
    Double(f64),
    Float(f32),
    Int(i32),
    Long(i64),
    Short(i16),
    Boolean(u8)
}

unsafe fn call_java_function_inner(env: *mut JNIEnv, this: Option<jobject>, method: &'static Method, class: &'static Class, args: Vec<JavaType>) -> Option<JavaReturnType> {
    let thread = get_thread(env).as_mut().unwrap();
    let this = if let Some(this) = this {Some((this as *mut JniRef).get_ref())} else {None};
    let method = class.resolve_method(&method.name, &method.descriptor).unwrap();
    if let Some(ret) = thread.call_from_jni(method, this, args) {
        Some(match ret {
            JavaType::Reference {..} => JavaReturnType::Object(thread.create_jni_local(ret) as jobject),
            JavaType::Byte(val) => JavaReturnType::Byte(val),
            JavaType::Char(val) => JavaReturnType::Char(val),
            JavaType::Double(val) => JavaReturnType::Double(val),
            JavaType::Float(val) => JavaReturnType::Float(val),
            JavaType::Int(val) => JavaReturnType::Int(val),
            JavaType::Long(val) => JavaReturnType::Long(val),
            JavaType::Short(val) => JavaReturnType::Short(val),
            JavaType::Boolean(val) => JavaReturnType::Boolean(if val {JNI_TRUE} else {JNI_FALSE}),
            _ => panic!()
        })
    } else {
        None
    }
}

unsafe fn call_java_function(env: *mut JNIEnv, this: Option<jobject>, method: jmethodID, class: &'static Class, mut args: VaListImpl) -> Option<JavaReturnType> {
    let method = method as *const Method;
    let method = method.as_ref().unwrap();
    let num_args = method.parameters.len();
    let mut args_vec = Vec::with_capacity(num_args);
    for arg_type in method.parameters.iter() {
        let arg = match arg_type.as_str() {
            "B" => JavaType::Byte(args.arg()),
            "C" => JavaType::Char(args.arg()),
            "D" => JavaType::Double(args.arg()),
            "F" => JavaType::Float(f32::from_bits(args.arg())),
            "I" => JavaType::Int(args.arg()),
            "J" => JavaType::Long(args.arg()),
            "S" => JavaType::Short(args.arg()),
            "Z" => JavaType::Boolean(args.arg::<u8>() != JNI_FALSE),
            _ => (args.arg::<jobject>() as *mut JniRef).get_ref()
        };
        args_vec.push(arg);
    }
    call_java_function_inner(env, this, method, class, args_vec)
}

unsafe fn call_java_function_v(env: *mut JNIEnv, this: Option<jobject>, method: jmethodID, class: &'static Class, mut args: va_list) -> Option<JavaReturnType> {
    let method = method as *const Method;
    let method = method.as_ref().unwrap();
    let num_args = method.parameters.len();
    let mut args_vec = Vec::with_capacity(num_args);
    for arg_type in method.parameters.iter() {
        let arg = match arg_type.as_str() {
            "B" => JavaType::Byte(args.get::<i32>() as i8),
            "C" => JavaType::Char(args.get::<u32>() as u16),
            "D" => JavaType::Double(f64::from_bits(args.get())),
            "F" => JavaType::Float(f32::from_bits(args.get())),
            "I" => JavaType::Int(args.get()),
            "J" => JavaType::Long(args.get()),
            "S" => JavaType::Short(args.get::<i32>() as i16),
            "Z" => JavaType::Boolean(args.get::<u32>() as u8 != JNI_FALSE),
            _ => (args.get::<*const JniRef>() as *mut JniRef).get_ref()
        };
        args_vec.push(arg);
    }
    call_java_function_inner(env, this, method, class, args_vec)
}

unsafe fn call_java_function_a(env: *mut JNIEnv, this: Option<jobject>, method: jmethodID, class: &'static Class, args: *const jvalue) -> Option<JavaReturnType> {
    let method = method as *const Method;
    let method = method.as_ref().unwrap();
    let num_args = method.parameters.len();
    let mut args_vec = Vec::with_capacity(num_args);
    let mut curr = args;
    for arg_type in method.parameters.iter() {
        let arg = match arg_type.as_str() {
            "B" => JavaType::Byte((*args).b),
            "C" => JavaType::Char((*args).c),
            "D" => JavaType::Double((*args).d),
            "F" => JavaType::Float((*args).f),
            "I" => JavaType::Int((*args).i),
            "J" => JavaType::Long((*args).j),
            "S" => JavaType::Short((*args).s),
            "Z" => JavaType::Boolean((*args).z != JNI_FALSE),
            _ => ((*args).l as *mut JniRef).get_ref()
        };
        args_vec.push(arg);
        curr = curr.add(1);
    }
    call_java_function_inner(env, this, method, class, args_vec)
}

unsafe extern "C" fn GetVersion(env: *mut JNIEnv) -> jint {
    0x00010008
}

unsafe extern "C" fn DefineClass(env: *mut JNIEnv, name: *const ::std::os::raw::c_char, loader: jobject, buf: *const jbyte, len: jsize) -> jclass {
    let buf = Vec::with_capacity(len as usize);
    let jc = JavaClass::new_from_bytes(buf);
    let jc = if let Ok(jc) = jc {
        jc
    } else {
        jni_exception!("ClassFormatError");
        return NULL;
    };
    if jc.get_name().starts_with("java/") {
        jni_exception!("SecurityException");
        return NULL;
    }
    let jc = Box::new(jc);
    let c = ::jvm::load_class_from_binary(jc);
    let c = if let Some(c) = c {
        c
    } else {
        jni_exception!("ClassCircularityError");
        return NULL;
    };
    let cobj = c.get_class_obj();
    create_local(env, cobj)
}

unsafe extern "C" fn FindClass(env: *mut JNIEnv, name: *const ::std::os::raw::c_char) -> jclass {
    let c = ::jvm::get_or_load_class(from_cstr(name));
    //TODO: more errors
    let c = if let Some(c) = c {
        c
    } else {
        jni_exception!("NoClassDefFoundError");
        return NULL;
    };
    create_local(env, c.get_class_obj())
}

unsafe extern "C" fn FromReflectedMethod(env: *mut JNIEnv, method: jobject) -> jmethodID {
    todo!()
}
unsafe extern "C" fn FromReflectedField(env: *mut JNIEnv, field: jobject) -> jfieldID {
    todo!()
}
unsafe extern "C" fn ToReflectedMethod(env: *mut JNIEnv,
                            cls: jclass,
                            methodID: jmethodID,
                            isStatic: jboolean)
                            -> jobject {
    todo!()
}
unsafe extern "C" fn GetSuperclass(env: *mut JNIEnv, sub: jclass) -> jclass {
    let class = get_class_from_jclass(sub);
    if let Some(sup) = class.super_class {
        create_local(env, sup.get_class_obj())
    } else {
        NULL
    }
}
unsafe extern "C" fn IsAssignableFrom(env: *mut JNIEnv,
                            sub: jclass,
                            sup: jclass)
                            -> jboolean {
    let sub = get_class_from_jclass(sub);
    let sup = get_class_from_jclass(sup);
    if sub.instanceof(sup) {JNI_TRUE} else {JNI_FALSE}
}
unsafe extern "C" fn ToReflectedField(env: *mut JNIEnv,
                            cls: jclass,
                            fieldID: jfieldID,
                            isStatic: jboolean)
                            -> jobject {
    todo!()
}
unsafe extern "C" fn Throw(env: *mut JNIEnv, obj: jthrowable) -> jint {
    todo!()
}
unsafe extern "C" fn ThrowNew(env: *mut JNIEnv,
                            clazz: jclass,
                            msg: *const ::std::os::raw::c_char)
                            -> jint {
    todo!()
}
unsafe extern "C" fn ExceptionOccurred(env: *mut JNIEnv) -> jthrowable {
    let thread = get_thread(env).as_mut().unwrap();
    if let Some(ex) = &thread.pending_exception {
        create_local(env, ex.clone()) as jthrowable
    } else {
        NULL
    }
}
unsafe extern "C" fn ExceptionDescribe(env: *mut JNIEnv) {
    todo!()
}
unsafe extern "C" fn ExceptionClear(env: *mut JNIEnv) {
    let thread = get_thread(env).as_mut().unwrap();
    if thread.pending_exception.is_some() {
        thread.pending_exception = None;
    }
}
unsafe extern "C" fn FatalError(env: *mut JNIEnv,
                            msg: *const ::std::os::raw::c_char) {
    panic!(from_cstr(msg))
}
unsafe extern "C" fn PushLocalFrame(env: *mut JNIEnv, capacity: jint) -> jint {
    get_thread(env).as_mut().unwrap().push_jni_frame();
    0
}
unsafe extern "C" fn PopLocalFrame(env: *mut JNIEnv, result: jobject) -> jobject {
    let thread = get_thread(env).as_mut().unwrap();
    let mut ans = NULL;
    if result != NULL {
        let result = (result as *mut JniRef).get_ref();
        let arc = if let JavaType::Reference {val, ..} = result {
            val
        } else {
            panic!();
        };
        let stack = thread.jni_stack.last().unwrap();
        let frame = &stack.frames[stack.frames.len()-2];
        for l in &frame.locals {
            let arc2 = if let JavaType::Reference {val, ..} = l.get_ref() {
                val
            } else {
                panic!()
            };
            if Arc::ptr_eq(&arc, &arc2) {
                ans = *l as jobject;
                break;
            }
        }
    }
    thread.pop_jni_frame();
    ans
}
unsafe extern "C" fn NewGlobalRef(env: *mut JNIEnv, lobj: jobject) -> jobject {
    let thread = get_thread(env);
    thread.as_mut().unwrap().create_jni_global((lobj as *mut JniRef).get_ref()) as jobject
}
unsafe extern "C" fn DeleteGlobalRef(env: *mut JNIEnv, gref: jobject) {
    let thread = get_thread(env);
    thread.as_mut().unwrap().delete_jni_global(gref as *mut JniRef)
}
unsafe extern "C" fn DeleteLocalRef(env: *mut JNIEnv, obj: jobject) {
    let thread = get_thread(env);
    thread.as_mut().unwrap().delete_jni_local(obj as *mut JniRef)
}
unsafe extern "C" fn IsSameObject(env: *mut JNIEnv,
                            obj1: jobject,
                            obj2: jobject)
                            -> jboolean {
    let obj1 = obj1 as *mut JniRef;
    let obj2 = obj2 as *mut JniRef;
    let arc = if let JavaType::Reference {val, ..} = obj1.get_ref() {
        val
    } else {
        return 0;
    };
    let arc2 = if let JavaType::Reference {val, ..} = obj2.get_ref() {
        val
    } else {
        return 0;
    };
    if Arc::ptr_eq(&arc, &arc2) {JNI_TRUE} else {JNI_FALSE}
}
unsafe extern "C" fn NewLocalRef(env: *mut JNIEnv, ref_: jobject) -> jobject {
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local((ref_ as *mut JniRef).get_ref()) as jobject
}
unsafe extern "C" fn EnsureLocalCapacity(env: *mut JNIEnv, capacity: jint) -> jint {
    0
}
unsafe extern "C" fn AllocObject(env: *mut JNIEnv, clazz: jclass) -> jobject {
    let class = get_class_from_jclass(clazz);
    let obj = class.instantiate();
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(obj) as jobject
}
unsafe extern "C" fn NewObject(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jobject {
    let class = get_class_from_jclass(clazz);
    let obj = class.instantiate();
    let thread = get_thread(env).as_mut().unwrap();
    let ans = thread.create_jni_local(obj) as jobject;
    call_java_function(env, Some(ans), methodID, class, args);
    ans
}
unsafe extern "C" fn NewObjectV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jobject {
    let class = get_class_from_jclass(clazz);
    let obj = class.instantiate();
    let thread = get_thread(env).as_mut().unwrap();
    let ans = thread.create_jni_local(obj) as jobject;
    call_java_function_v(env, Some(ans), methodID, class, args);
    ans
}
unsafe extern "C" fn NewObjectA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jobject {
    let class = get_class_from_jclass(clazz);
    let obj = class.instantiate();
    let thread = get_thread(env).as_mut().unwrap();
    let ans = thread.create_jni_local(obj) as jobject;
    call_java_function_a(env, Some(ans), methodID, class, args);
    ans
}
unsafe extern "C" fn GetObjectClass(env: *mut JNIEnv, obj: jobject) -> jclass {
    let thread = get_thread(env).as_mut().unwrap();
    let obj = (obj as *mut JniRef).get_ref();
    let class = if let JavaType::Reference {class, ..} = obj {
        class
    } else {
        panic!();
    };
    thread.create_jni_local(class.get_class_obj()) as jclass
}
unsafe extern "C" fn IsInstanceOf(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass)
                            -> jboolean {
    let obj = (obj as *mut JniRef).get_ref();
    let class = if let JavaType::Reference {class, ..} = obj {
        class
    } else {
        panic!();
    };
    let clazz = get_class_from_jclass(clazz);
    if class.instanceof(clazz) {JNI_TRUE} else {JNI_FALSE}
}
unsafe extern "C" fn GetMethodID(env: *mut JNIEnv,
                            clazz: jclass,
                            name: *const ::std::os::raw::c_char,
                            sig: *const ::std::os::raw::c_char)
                            -> jmethodID {
    let class = get_class_from_jclass(clazz);
    let name = from_cstr(name);
    let sig = from_cstr(sig);
    let repr = name.to_owned()+sig;
    let mut curr = Some(class);
    while let Some(c) = curr {
        if let Some(&m) = c.methods.get(&repr) {
            return m as *const Method as jmethodID;
        } 
        curr = c.super_class;
    }
    NULL as jmethodID
}
unsafe extern "C" fn CallObjectMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallObjectMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallObjectMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallBooleanMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallBooleanMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallBooleanMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallByteMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallByteMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallByteMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallCharMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallCharMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallCharMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallShortMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallShortMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallShortMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallIntMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallIntMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallIntMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallLongMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallLongMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallLongMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallFloatMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallFloatMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallFloatMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallDoubleMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallDoubleMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallDoubleMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallVoidMethod(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: ...) {
    call_java_function(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args);
}
unsafe extern "C" fn CallVoidMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: va_list) {
    call_java_function_v(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args);
}
unsafe extern "C" fn CallVoidMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            methodID: jmethodID,
                            args: *const jvalue) {
    call_java_function_a(env, Some(obj), methodID, (obj as *mut JniRef).get_class(), args);
}
unsafe extern "C" fn CallNonvirtualObjectMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualObjectMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualObjectMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualBooleanMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualBooleanMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualBooleanMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualByteMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualByteMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualByteMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualCharMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualCharMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualCharMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualShortMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualShortMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualShortMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualIntMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualIntMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualIntMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualLongMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualLongMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualLongMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualFloatMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualFloatMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualFloatMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualDoubleMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualDoubleMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualDoubleMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallNonvirtualVoidMethod(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...) {
    call_java_function(env, Some(obj), methodID, get_class_from_jclass(clazz), args);
}
unsafe extern "C" fn CallNonvirtualVoidMethodV(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list) {
    call_java_function_v(env, Some(obj), methodID, get_class_from_jclass(clazz), args);
}
unsafe extern "C" fn CallNonvirtualVoidMethodA(env: *mut JNIEnv,
                            obj: jobject,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue) {
    call_java_function_a(env, Some(obj), methodID, get_class_from_jclass(clazz), args);
}
unsafe extern "C" fn GetFieldID(env: *mut JNIEnv,
                            clazz: jclass,
                            name: *const ::std::os::raw::c_char,
                            sig: *const ::std::os::raw::c_char)
                            -> jfieldID {
    let class = get_class_from_jclass(clazz);
    ensure_class_init(class);
    let name = from_cstr(name);
    let sig = from_cstr(sig);
    let repr = name.to_owned() + sig;
    for f in &class.instance_fields {
        if f.name == name && f.descriptor_raw == sig {
            return f as *const InstanceFieldInfo as jfieldID;
        }
    }
    NULL as jfieldID
}
unsafe extern "C" fn GetObjectField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jobject {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if matches!(ans, JavaType::Reference {..}) {
        let thread = get_thread(env).as_mut().unwrap();
        thread.create_jni_local(ans) as jobject
    } else {
        panic!();
    }
}
unsafe extern "C" fn GetBooleanField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jboolean {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Boolean(b) = ans {
        if b {JNI_TRUE} else {JNI_FALSE}
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetByteField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jbyte {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Byte(val) = ans {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetCharField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jchar {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Char(val) = ans {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetShortField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jshort {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Short(val) = ans {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetIntField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jint {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Int(val) = ans {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetLongField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jlong {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Long(val) = ans {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetFloatField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jfloat {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Float(val) = ans {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetDoubleField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID)
                            -> jdouble {
    let obj = (obj as *mut JniRef).get_ref();
    let ans = obj.get_field((fieldID as *const InstanceFieldInfo).as_ref().unwrap().name);
    if let JavaType::Double(val) = ans {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn SetObjectField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jobject) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = (val as *mut JniRef).get_ref();
    obj.set_field(name, val);
}
unsafe extern "C" fn SetBooleanField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jboolean) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Boolean(val != JNI_FALSE);
    obj.set_field(name, val);
}
unsafe extern "C" fn SetByteField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jbyte) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Byte(val);
    obj.set_field(name, val);
}
unsafe extern "C" fn SetCharField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jchar) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Char(val);
    obj.set_field(name, val);
}
unsafe extern "C" fn SetShortField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jshort) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Short(val);
    obj.set_field(name, val);
}
unsafe extern "C" fn SetIntField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jint) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Int(val);
    obj.set_field(name, val);
}
unsafe extern "C" fn SetLongField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jlong) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Long(val);
    obj.set_field(name, val);
}
unsafe extern "C" fn SetFloatField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jfloat) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Float(val);
    obj.set_field(name, val);
}
unsafe extern "C" fn SetDoubleField(env: *mut JNIEnv,
                            obj: jobject,
                            fieldID: jfieldID,
                            val: jdouble) {
    let obj = (obj as *mut JniRef).get_ref();
    let name = (fieldID as *const InstanceFieldInfo).as_ref().unwrap().name;
    let val = JavaType::Double(val);
    obj.set_field(name, val);
}
unsafe extern "C" fn GetStaticMethodID(env: *mut JNIEnv,
                            clazz: jclass,
                            name: *const ::std::os::raw::c_char,
                            sig: *const ::std::os::raw::c_char)
                            -> jmethodID {
    let class = get_class_from_jclass(clazz);
    ensure_class_init(class);
    let name = from_cstr(name);
    let sig = from_cstr(sig);
    if let Ok(m) = class.resolve_static_method(name, sig) {
        m as *const Method as jmethodID
    } else {
        NULL as jmethodID
    }
}
unsafe extern "C" fn CallStaticObjectMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticObjectMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticObjectMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jobject {
    if let Some(JavaReturnType::Object(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticBooleanMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticBooleanMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticBooleanMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jboolean {
    if let Some(JavaReturnType::Boolean(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticByteMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticByteMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticByteMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jbyte {
    if let Some(JavaReturnType::Byte(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticCharMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticCharMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticCharMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jchar {
    if let Some(JavaReturnType::Char(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticShortMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticShortMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticShortMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jshort {
    if let Some(JavaReturnType::Short(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticIntMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticIntMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticIntMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jint {
    if let Some(JavaReturnType::Int(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticLongMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticLongMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticLongMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jlong {
    if let Some(JavaReturnType::Long(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticFloatMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticFloatMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticFloatMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jfloat {
    if let Some(JavaReturnType::Float(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticDoubleMethod(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: ...)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticDoubleMethodV(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: va_list)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function_v(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticDoubleMethodA(env: *mut JNIEnv,
                            clazz: jclass,
                            methodID: jmethodID,
                            args: *const jvalue)
                            -> jdouble {
    if let Some(JavaReturnType::Double(ans)) = call_java_function_a(env, None, methodID, get_class_from_jclass(clazz), args) {
        ans
    } else {
        panic!();
    }
}
unsafe extern "C" fn CallStaticVoidMethod(env: *mut JNIEnv,
                            cls: jclass,
                            methodID: jmethodID,
                            args: ...) {
    call_java_function(env, None, methodID, get_class_from_jclass(cls), args);
}
unsafe extern "C" fn CallStaticVoidMethodV(env: *mut JNIEnv,
                            cls: jclass,
                            methodID: jmethodID,
                            args: va_list) {
    call_java_function_v(env, None, methodID, get_class_from_jclass(cls), args);
}
unsafe extern "C" fn CallStaticVoidMethodA(env: *mut JNIEnv,
                            cls: jclass,
                            methodID: jmethodID,
                            args: *const jvalue) {
    call_java_function_a(env, None, methodID, get_class_from_jclass(cls), args);
}
unsafe extern "C" fn GetStaticFieldID(env: *mut JNIEnv,
                            clazz: jclass,
                            name: *const ::std::os::raw::c_char,
                            sig: *const ::std::os::raw::c_char)
                            -> jfieldID {
    let class = get_class_from_jclass(clazz);
    ensure_class_init(class);
    let name = from_cstr(name);
    let sig = from_cstr(sig);
    let repr = name.to_owned() + sig;
    if let Some(f) = class.fields.get(&repr) {
        f as *const Field as jfieldID
    } else {
        NULL as jfieldID
    }
}
unsafe extern "C" fn GetStaticObjectField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jobject {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = f.value.read().unwrap().clone();
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(val) as jobject
}
unsafe extern "C" fn GetStaticBooleanField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jboolean {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Boolean(val) =  *f.value.read().unwrap() {
        if val {JNI_TRUE} else {JNI_FALSE}
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetStaticByteField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jbyte {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Byte(val) =  *f.value.read().unwrap() {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetStaticCharField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jchar {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Char(val) =  *f.value.read().unwrap() {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetStaticShortField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jshort {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Short(val) =  *f.value.read().unwrap() {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetStaticIntField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jint {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Int(val) =  *f.value.read().unwrap() {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetStaticLongField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jlong {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Long(val) =  *f.value.read().unwrap() {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetStaticFloatField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jfloat {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Float(val) =  *f.value.read().unwrap() {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn GetStaticDoubleField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID)
                            -> jdouble {
    let f = (fieldID as *const Field).as_ref().unwrap();
    if let JavaType::Double(val) =  *f.value.read().unwrap() {
        val
    } else {
        panic!()
    }
}
unsafe extern "C" fn SetStaticObjectField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jobject) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = (value as *mut JniRef).get_ref();
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticBooleanField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jboolean) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Boolean(value != JNI_FALSE);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticByteField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jbyte) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Byte(value);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticCharField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jchar) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Char(value);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticShortField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jshort) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Short(value);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticIntField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jint) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Int(value);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticLongField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jlong) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Long(value);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticFloatField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jfloat) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Float(value);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn SetStaticDoubleField(env: *mut JNIEnv,
                            clazz: jclass,
                            fieldID: jfieldID,
                            value: jdouble) {
    let f = (fieldID as *const Field).as_ref().unwrap();
    let val = JavaType::Double(value);
    *(f.value.write().unwrap()) = val;
}
unsafe extern "C" fn NewString(env: *mut JNIEnv,
                            unicode: *const jchar,
                            len: jsize)
                            -> jstring {
    let mut arr = Vec::with_capacity(len as usize);
    for i in 0..len {
        arr.push(*unicode.add(i as usize));
    }
    let string = String::from_utf16(&arr).unwrap();
    let ans = get_or_intern_string(string);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(ans) as jstring
}
unsafe extern "C" fn GetStringLength(env: *mut JNIEnv, str: jstring) -> jsize {
    let obj = (str as *mut JniRef).get_ref();
    let arr = obj.get_field("value");
    arr.array_length()
}

#[repr(C)]
struct StringChars {
    raw_box: *mut [u16],
    ptr: *const u16
}
unsafe extern "C" fn GetStringChars(env: *mut JNIEnv,
                            str: jstring,
                            isCopy: *mut jboolean)
                            -> *const jchar {
    let obj = (str as *mut JniRef).get_ref();
    let arr = obj.get_field("value");
    if let JavaType::Reference {val, ..} = arr {
        if let JavaType::Array {data, ..} = &*val.read().unwrap() {
            let mut ans = Vec::with_capacity(data.len());
            for c in data.iter() {
                if let JavaType::Char(val) = c {
                    ans.push(*val);
                } else {
                    panic!()
                }
            }
            let ptr = ans.as_mut_ptr();
            mem::forget(ans);
            if !isCopy.is_null() {
                *isCopy = 1;
            }
            ptr
        } else {
            panic!()
        }
    } else {
        panic!()
    }
}
unsafe extern "C" fn ReleaseStringChars(env: *mut JNIEnv,
                            str: jstring,
                            chars: *const jchar) {
    let obj = (str as *mut JniRef).get_ref();
    let arr = obj.get_field("value");
    let len = arr.array_length() as usize;
    let vec = Vec::from_raw_parts(chars as *mut u16, len, len);
}
unsafe extern "C" fn NewStringUTF(env: *mut JNIEnv,
                            utf: *const ::std::os::raw::c_char)
                            -> jstring {
    let str = from_cstr(utf).to_owned();
    let ans = get_or_intern_string(str);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(ans) as jstring
}
unsafe extern "C" fn GetStringUTFLength(env: *mut JNIEnv, str: jstring) -> jsize {
    let obj = (str as *mut JniRef).get_ref();
    let utf16 = obj.clone_arr_data();
    let utf16: Vec<u16> = utf16.iter().map(|c| if let JavaType::Char(c) = c {*c} else {panic!()}).collect();
    let str = String::from_utf16(&utf16).unwrap();
    str.len() as jsize
}
unsafe extern "C" fn GetStringUTFChars(env: *mut JNIEnv,
                            str: jstring,
                            isCopy: *mut jboolean)
                            -> *const ::std::os::raw::c_char {
    let obj = (str as *mut JniRef).get_ref();
    let utf16 = obj.clone_arr_data();
    let utf16: Vec<u16> = utf16.iter().map(|c| if let JavaType::Char(c) = c {*c} else {panic!()}).collect();
    let str = String::from_utf16(&utf16).unwrap();
    let cstr = CString::from_vec_unchecked(str.into());
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    cstr.into_raw()
}
unsafe extern "C" fn ReleaseStringUTFChars(env: *mut JNIEnv,
                            str: jstring,
                            chars: *const ::std::os::raw::c_char) {
    CString::from_raw(chars as *mut i8);
}
unsafe extern "C" fn GetArrayLength(env: *mut JNIEnv, array: jarray) -> jsize {
    let array = (array as *mut JniRef).get_ref();
    array.array_length()
}
unsafe extern "C" fn NewObjectArray(env: *mut JNIEnv,
                            len: jsize,
                            clazz: jclass,
                            init: jobject)
                            -> jobjectArray {
    let class = get_class_from_jclass(clazz);
    let arr = ::jvm::create_array(class, len as usize);
    let obj = (init as *mut JniRef).get_ref();
    arr.arr_fill(&obj);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn GetObjectArrayElement(env: *mut JNIEnv,
                            array: jobjectArray,
                            index: jsize)
                            -> jobject {
    let array = (array as *mut JniRef).get_ref();
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(array.array_get(index as usize)) as jobject
}
unsafe extern "C" fn SetObjectArrayElement(env: *mut JNIEnv,
                            array: jobjectArray,
                            index: jsize,
                            val: jobject) {
    let array = (array as *mut JniRef).get_ref();
    array.array_set(index as usize, (val as *mut JniRef).get_ref());
}
unsafe extern "C" fn NewBooleanArray(env: *mut JNIEnv, len: jsize) -> jbooleanArray {
    let class = ::jvm::get_or_load_class("Z").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn NewByteArray(env: *mut JNIEnv, len: jsize) -> jbyteArray {
    let class = ::jvm::get_or_load_class("B").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn NewCharArray(env: *mut JNIEnv, len: jsize) -> jcharArray {
    let class = ::jvm::get_or_load_class("C").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn NewShortArray(env: *mut JNIEnv, len: jsize) -> jshortArray {
    let class = ::jvm::get_or_load_class("S").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn NewIntArray(env: *mut JNIEnv, len: jsize) -> jintArray {
    let class = ::jvm::get_or_load_class("I").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn NewLongArray(env: *mut JNIEnv, len: jsize) -> jlongArray {
    let class = ::jvm::get_or_load_class("J").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn NewFloatArray(env: *mut JNIEnv, len: jsize) -> jfloatArray {
    let class = ::jvm::get_or_load_class("F").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn NewDoubleArray(env: *mut JNIEnv, len: jsize) -> jdoubleArray {
    let class = ::jvm::get_or_load_class("D").unwrap();
    let arr = ::jvm::create_array(class, len as usize);
    let thread = get_thread(env).as_mut().unwrap();
    thread.create_jni_local(arr) as jobject
}
unsafe extern "C" fn GetBooleanArrayElements(env: *mut JNIEnv,
                            array: jbooleanArray,
                            isCopy: *mut jboolean)
                            -> *mut jboolean {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans = data.iter().map(|e| if let JavaType::Boolean(val) = e {if *val {JNI_TRUE} else {JNI_FALSE}} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn GetByteArrayElements(env: *mut JNIEnv,
                            array: jbyteArray,
                            isCopy: *mut jboolean)
                            -> *mut jbyte {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans = data.iter().map(|e| if let JavaType::Byte(val) = e {*val} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn GetCharArrayElements(env: *mut JNIEnv,
                            array: jcharArray,
                            isCopy: *mut jboolean)
                            -> *mut jchar {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans  = data.iter().map(|e| if let JavaType::Char(val) = e {*val} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn GetShortArrayElements(env: *mut JNIEnv,
                            array: jshortArray,
                            isCopy: *mut jboolean)
                            -> *mut jshort {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans = data.iter().map(|e| if let JavaType::Short(val) = e {*val} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn GetIntArrayElements(env: *mut JNIEnv,
                            array: jintArray,
                            isCopy: *mut jboolean)
                            -> *mut jint {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans = data.iter().map(|e| if let JavaType::Int(val) = e {*val} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn GetLongArrayElements(env: *mut JNIEnv,
                            array: jlongArray,
                            isCopy: *mut jboolean)
                            -> *mut jlong {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans = data.iter().map(|e| if let JavaType::Long(val) = e {*val} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn GetFloatArrayElements(env: *mut JNIEnv,
                            array: jfloatArray,
                            isCopy: *mut jboolean)
                            -> *mut jfloat {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans = data.iter().map(|e| if let JavaType::Float(val) = e {*val} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn GetDoubleArrayElements(env: *mut JNIEnv,
                            array: jdoubleArray,
                            isCopy: *mut jboolean)
                            -> *mut jdouble {
    let array = (array as *mut JniRef).get_ref();
    let data = array.clone_arr_data();
    let ans = data.iter().map(|e| if let JavaType::Double(val) = e {*val} else {panic!()}).collect();
    if !isCopy.is_null() {
        *isCopy = 1;
    }
    Vec::into_raw_parts(ans).0
}
unsafe extern "C" fn ReleaseBooleanArrayElements(env: *mut JNIEnv,
                            array: jbooleanArray,
                            elems: *mut jboolean,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Boolean(*val != JNI_FALSE)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn ReleaseByteArrayElements(env: *mut JNIEnv,
                            array: jbyteArray,
                            elems: *mut jbyte,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Byte(*val)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn ReleaseCharArrayElements(env: *mut JNIEnv,
                            array: jcharArray,
                            elems: *mut jchar,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Char(*val)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn ReleaseShortArrayElements(env: *mut JNIEnv,
                            array: jshortArray,
                            elems: *mut jshort,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Short(*val)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn ReleaseIntArrayElements(env: *mut JNIEnv,
                            array: jintArray,
                            elems: *mut jint,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Int(*val)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn ReleaseLongArrayElements(env: *mut JNIEnv,
                            array: jlongArray,
                            elems: *mut jlong,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Long(*val)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn ReleaseFloatArrayElements(env: *mut JNIEnv,
                            array: jfloatArray,
                            elems: *mut jfloat,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Float(*val)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn ReleaseDoubleArrayElements(env: *mut JNIEnv,
                            array: jdoubleArray,
                            elems: *mut jdouble,
                            mode: jint) {
    let array = (array as *mut JniRef).get_ref();
    let len = array.array_length() as usize;
    let vec = Vec::from_raw_parts(elems, len, len);
    if mode != JNI_ABORT {
        let vec: Vec<JavaType> = vec.iter().map(|val| JavaType::Double(*val)).collect();
        array.arr_set_all(&vec);
    }
    if mode == JNI_COMMIT {
        vec.into_raw_parts();
    } else {
        drop(vec); //technically not needed but it makes the intent more clear
    }
}
unsafe extern "C" fn GetBooleanArrayRegion(env: *mut JNIEnv,
                            array: jbooleanArray,
                            start: jsize,
                            l: jsize,
                            buf: *mut jboolean) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..l as usize {
        *(buf.add(i)) = if (&array[start as usize + i]).unwrap() {JNI_TRUE} else {JNI_FALSE};
    }
}
unsafe extern "C" fn GetByteArrayRegion(env: *mut JNIEnv,
                            array: jbyteArray,
                            start: jsize,
                            len: jsize,
                            buf: *mut jbyte) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn GetCharArrayRegion(env: *mut JNIEnv,
                            array: jcharArray,
                            start: jsize,
                            len: jsize,
                            buf: *mut jchar) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn GetShortArrayRegion(env: *mut JNIEnv,
                            array: jshortArray,
                            start: jsize,
                            len: jsize,
                            buf: *mut jshort) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn GetIntArrayRegion(env: *mut JNIEnv,
                            array: jintArray,
                            start: jsize,
                            len: jsize,
                            buf: *mut jint) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn GetLongArrayRegion(env: *mut JNIEnv,
                            array: jlongArray,
                            start: jsize,
                            len: jsize,
                            buf: *mut jlong) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn GetFloatArrayRegion(env: *mut JNIEnv,
                            array: jfloatArray,
                            start: jsize,
                            len: jsize,
                            buf: *mut jfloat) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn GetDoubleArrayRegion(env: *mut JNIEnv,
                            array: jdoubleArray,
                            start: jsize,
                            len: jsize,
                            buf: *mut jdouble) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn SetBooleanArrayRegion(env: *mut JNIEnv,
                            array: jbooleanArray,
                            start: jsize,
                            l: jsize,
                            buf: *const jboolean) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..l as usize {
        array.array_set(start + i, (*(buf.add(i)) != JNI_FALSE).into());
    }
}
unsafe extern "C" fn SetByteArrayRegion(env: *mut JNIEnv,
                            array: jbyteArray,
                            start: jsize,
                            len: jsize,
                            buf: *const jbyte) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..len as usize {
        array.array_set(start + i, (*(buf.add(i))).into());
    }
}
unsafe extern "C" fn SetCharArrayRegion(env: *mut JNIEnv,
                            array: jcharArray,
                            start: jsize,
                            len: jsize,
                            buf: *const jchar) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..len as usize {
        array.array_set(start + i, (*(buf.add(i))).into());
    }
}
unsafe extern "C" fn SetShortArrayRegion(env: *mut JNIEnv,
                            array: jshortArray,
                            start: jsize,
                            len: jsize,
                            buf: *const jshort) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..len as usize {
        array.array_set(start + i, (*(buf.add(i))).into());
    }
}
unsafe extern "C" fn SetIntArrayRegion(env: *mut JNIEnv,
                            array: jintArray,
                            start: jsize,
                            len: jsize,
                            buf: *const jint) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..len as usize {
        array.array_set(start + i, (*(buf.add(i))).into());
    }
}
unsafe extern "C" fn SetLongArrayRegion(env: *mut JNIEnv,
                            array: jlongArray,
                            start: jsize,
                            len: jsize,
                            buf: *const jlong) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..len as usize {
        array.array_set(start + i, (*(buf.add(i))).into());
    }
}
unsafe extern "C" fn SetFloatArrayRegion(env: *mut JNIEnv,
                            array: jfloatArray,
                            start: jsize,
                            len: jsize,
                            buf: *const jfloat) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..len as usize {
        array.array_set(start + i, (*(buf.add(i))).into());
    }
}
unsafe extern "C" fn SetDoubleArrayRegion(env: *mut JNIEnv,
                            array: jdoubleArray,
                            start: jsize,
                            len: jsize,
                            buf: *const jdouble) {
    let start = start as usize;
    let array = (array as *mut JniRef).get_ref();
    for i in 0..len as usize {
        array.array_set(start + i, (*(buf.add(i))).into());
    }
}
unsafe extern "C" fn RegisterNatives(env: *mut JNIEnv,
                            clazz: jclass,
                            methods: *const JNINativeMethod,
                            nMethods: jint)
                            -> jint {
    let n = nMethods as usize;
    let class = get_class_from_jclass(clazz);
    for i in 0..n {
        let info = *(methods.add(i));
        let name = from_cstr(info.name);
        let signature = from_cstr(info.signature);
        let repr = name.to_owned() + signature;
        let method = class.methods.get(&repr);
        if method.is_none() || !method.unwrap().is_native() {
            jni_exception!("NoSuchMethodError");
            return -1;
        }
        let method = *method.unwrap();
        *(method.native_fn.as_ref().unwrap().write().unwrap()) = info.fnPtr;
    }
    0
}
unsafe extern "C" fn UnregisterNatives(env: *mut JNIEnv, clazz: jclass) -> jint {
    todo!()
}
unsafe extern "C" fn MonitorEnter(env: *mut JNIEnv, obj: jobject) -> jint {
    todo!()
}
unsafe extern "C" fn MonitorExit(env: *mut JNIEnv, obj: jobject) -> jint {
    todo!()
}
unsafe extern "C" fn GetJavaVM(env: *mut JNIEnv, vm: *mut *mut JavaVM) -> jint {
    todo!()
}
unsafe extern "C" fn GetStringRegion(env: *mut JNIEnv,
                            str: jstring,
                            start: jsize,
                            len: jsize,
                            buf: *mut jchar) {
    let start = start as usize;
    let obj = (str as *mut JniRef).get_ref();
    let array = obj.get_field("value");
    let array = array.clone_arr_data();
    for i in 0..len as usize {
        *(buf.add(i)) = (&array[start as usize + i]).unwrap();
    }
}
unsafe extern "C" fn GetStringUTFRegion(env: *mut JNIEnv,
                            str: jstring,
                            start: jsize,
                            len: jsize,
                            buf: *mut ::std::os::raw::c_char) {
    let start = start as usize;
    let obj = (str as *mut JniRef).get_ref();
    let array = obj.get_field("value");
    let array: Vec<u16> = array.clone_arr_data().iter().map(JavaType::unwrap).collect();
    let str = String::from_utf16(&array).unwrap();
    let bytes = str.as_bytes();
    for i in 0..len as usize {
        *(buf.add(i)) = mem::transmute(bytes[start as usize + i]);
    }
}
unsafe extern "C" fn GetPrimitiveArrayCritical(env: *mut JNIEnv,
                            array: jarray,
                            isCopy: *mut jboolean)
                            -> *mut ::std::os::raw::c_void {
    todo!()
}
unsafe extern "C" fn ReleasePrimitiveArrayCritical(env: *mut JNIEnv,
                            array: jarray,
                            carray: *mut ::std::os::raw::c_void,
                            mode: jint) {
    todo!()
}
unsafe extern "C" fn GetStringCritical(env: *mut JNIEnv,
                            string: jstring,
                            isCopy: *mut jboolean)
                            -> *const jchar {
    todo!()
}
unsafe extern "C" fn ReleaseStringCritical(env: *mut JNIEnv,
                            string: jstring,
                            cstring: *const jchar) {
    todo!()
}
unsafe extern "C" fn NewWeakGlobalRef(env: *mut JNIEnv, obj: jobject) -> jweak {
    let obj = (obj as *mut JniRef).get_ref();
    JniRef::new_weak_global(obj) as jweak
}
unsafe extern "C" fn DeleteWeakGlobalRef(env: *mut JNIEnv, ref_: jweak) {
    JniRef::delete(ref_ as *mut JniRef);
}
unsafe extern "C" fn ExceptionCheck(env: *mut JNIEnv) -> jboolean {
    let thread = get_thread(env);
    if (*thread).pending_exception.is_some() {JNI_TRUE} else {JNI_FALSE}
}
unsafe extern "C" fn NewDirectByteBuffer(env: *mut JNIEnv,
                            address: *mut ::std::os::raw::c_void,
                            capacity: jlong)
                            -> jobject {
    todo!()
}
unsafe extern "C" fn GetDirectBufferAddress(env: *mut JNIEnv, buf: jobject)
                            -> *mut ::std::os::raw::c_void {
    todo!()
}
unsafe extern "C" fn GetDirectBufferCapacity(env: *mut JNIEnv, buf: jobject) -> jlong {
    todo!()
}
unsafe extern "C" fn GetObjectRefType(env: *mut JNIEnv,
                            obj: jobject)
                            -> jobjectRefType {
    match *(obj as *mut JniRef) {
        JniRef::Global(_) => 2,
        JniRef::Local(_) => 1,
        JniRef::WeakGlobal(_, _) => 3
    }
}

pub const JNI_FUNCTIONS: JNINativeInterface_ = JNINativeInterface_ {
    reserved0: 0 as *mut std::ffi::c_void,
    reserved1: 0 as *mut std::ffi::c_void,
    reserved2: 0 as *mut std::ffi::c_void,
    reserved3: 0 as *mut std::ffi::c_void,
    GetVersion: Some(GetVersion),
    DefineClass: Some(DefineClass),
    FindClass: Some(FindClass),
    FromReflectedMethod: Some(FromReflectedMethod),
    FromReflectedField: Some(FromReflectedField),
    ToReflectedMethod: Some(ToReflectedMethod),
    GetSuperclass: Some(GetSuperclass),
    IsAssignableFrom: Some(IsAssignableFrom),
    ToReflectedField: Some(ToReflectedField),
    Throw: Some(Throw),
    ThrowNew: Some(ThrowNew),
    ExceptionOccurred: Some(ExceptionOccurred),
    ExceptionDescribe: Some(ExceptionDescribe),
    ExceptionClear: Some(ExceptionClear),
    FatalError: Some(FatalError),
    PushLocalFrame: Some(PushLocalFrame),
    PopLocalFrame: Some(PopLocalFrame),
    NewGlobalRef: Some(NewGlobalRef),
    DeleteGlobalRef: Some(DeleteGlobalRef),
    DeleteLocalRef: Some(DeleteLocalRef),
    IsSameObject: Some(IsSameObject),
    NewLocalRef: Some(NewLocalRef),
    EnsureLocalCapacity: Some(EnsureLocalCapacity),
    AllocObject: Some(AllocObject),
    NewObject: Some(NewObject),
    NewObjectV: Some(NewObjectV),
    NewObjectA: Some(NewObjectA),
    GetObjectClass: Some(GetObjectClass),
    IsInstanceOf: Some(IsInstanceOf),
    GetMethodID: Some(GetMethodID),
    CallObjectMethod: Some(CallObjectMethod),
    CallObjectMethodV: Some(CallObjectMethodV),
    CallObjectMethodA: Some(CallObjectMethodA),
    CallBooleanMethod: Some(CallBooleanMethod),
    CallBooleanMethodV: Some(CallBooleanMethodV),
    CallBooleanMethodA: Some(CallBooleanMethodA),
    CallByteMethod: Some(CallByteMethod),
    CallByteMethodV: Some(CallByteMethodV),
    CallByteMethodA: Some(CallByteMethodA),
    CallCharMethod: Some(CallCharMethod),
    CallCharMethodV: Some(CallCharMethodV),
    CallCharMethodA: Some(CallCharMethodA),
    CallShortMethod: Some(CallShortMethod),
    CallShortMethodV: Some(CallShortMethodV),
    CallShortMethodA: Some(CallShortMethodA),
    CallIntMethod: Some(CallIntMethod),
    CallIntMethodV: Some(CallIntMethodV),
    CallIntMethodA: Some(CallIntMethodA),
    CallLongMethod: Some(CallLongMethod),
    CallLongMethodV: Some(CallLongMethodV),
    CallLongMethodA: Some(CallLongMethodA),
    CallFloatMethod: Some(CallFloatMethod),
    CallFloatMethodV: Some(CallFloatMethodV),
    CallFloatMethodA: Some(CallFloatMethodA),
    CallDoubleMethod: Some(CallDoubleMethod),
    CallDoubleMethodV: Some(CallDoubleMethodV),
    CallDoubleMethodA: Some(CallDoubleMethodA),
    CallVoidMethod: Some(CallVoidMethod),
    CallVoidMethodV: Some(CallVoidMethodV),
    CallVoidMethodA: Some(CallVoidMethodA),
    CallNonvirtualObjectMethod: Some(CallNonvirtualObjectMethod),
    CallNonvirtualObjectMethodV: Some(CallNonvirtualObjectMethodV),
    CallNonvirtualObjectMethodA: Some(CallNonvirtualObjectMethodA),
    CallNonvirtualBooleanMethod: Some(CallNonvirtualBooleanMethod),
    CallNonvirtualBooleanMethodV: Some(CallNonvirtualBooleanMethodV),
    CallNonvirtualBooleanMethodA: Some(CallNonvirtualBooleanMethodA),
    CallNonvirtualByteMethod: Some(CallNonvirtualByteMethod),
    CallNonvirtualByteMethodV: Some(CallNonvirtualByteMethodV),
    CallNonvirtualByteMethodA: Some(CallNonvirtualByteMethodA),
    CallNonvirtualCharMethod: Some(CallNonvirtualCharMethod),
    CallNonvirtualCharMethodV: Some(CallNonvirtualCharMethodV),
    CallNonvirtualCharMethodA: Some(CallNonvirtualCharMethodA),
    CallNonvirtualShortMethod: Some(CallNonvirtualShortMethod),
    CallNonvirtualShortMethodV: Some(CallNonvirtualShortMethodV),
    CallNonvirtualShortMethodA: Some(CallNonvirtualShortMethodA),
    CallNonvirtualIntMethod: Some(CallNonvirtualIntMethod),
    CallNonvirtualIntMethodV: Some(CallNonvirtualIntMethodV),
    CallNonvirtualIntMethodA: Some(CallNonvirtualIntMethodA),
    CallNonvirtualLongMethod: Some(CallNonvirtualLongMethod),
    CallNonvirtualLongMethodV: Some(CallNonvirtualLongMethodV),
    CallNonvirtualLongMethodA: Some(CallNonvirtualLongMethodA),
    CallNonvirtualFloatMethod: Some(CallNonvirtualFloatMethod),
    CallNonvirtualFloatMethodV: Some(CallNonvirtualFloatMethodV),
    CallNonvirtualFloatMethodA: Some(CallNonvirtualFloatMethodA),
    CallNonvirtualDoubleMethod: Some(CallNonvirtualDoubleMethod),
    CallNonvirtualDoubleMethodV: Some(CallNonvirtualDoubleMethodV),
    CallNonvirtualDoubleMethodA: Some(CallNonvirtualDoubleMethodA),
    CallNonvirtualVoidMethod: Some(CallNonvirtualVoidMethod),
    CallNonvirtualVoidMethodV: Some(CallNonvirtualVoidMethodV),
    CallNonvirtualVoidMethodA: Some(CallNonvirtualVoidMethodA),
    GetFieldID: Some(GetFieldID),
    GetObjectField: Some(GetObjectField),
    GetBooleanField: Some(GetBooleanField),
    GetByteField: Some(GetByteField),
    GetCharField: Some(GetCharField),
    GetShortField: Some(GetShortField),
    GetIntField: Some(GetIntField),
    GetLongField: Some(GetLongField),
    GetFloatField: Some(GetFloatField),
    GetDoubleField: Some(GetDoubleField),
    SetObjectField: Some(SetObjectField),
    SetBooleanField: Some(SetBooleanField),
    SetByteField: Some(SetByteField),
    SetCharField: Some(SetCharField),
    SetShortField: Some(SetShortField),
    SetIntField: Some(SetIntField),
    SetLongField: Some(SetLongField),
    SetFloatField: Some(SetFloatField),
    SetDoubleField: Some(SetDoubleField),
    GetStaticMethodID: Some(GetStaticMethodID),
    CallStaticObjectMethod: Some(CallStaticObjectMethod),
    CallStaticObjectMethodV: Some(CallStaticObjectMethodV),
    CallStaticObjectMethodA: Some(CallStaticObjectMethodA),
    CallStaticBooleanMethod: Some(CallStaticBooleanMethod),
    CallStaticBooleanMethodV: Some(CallStaticBooleanMethodV),
    CallStaticBooleanMethodA: Some(CallStaticBooleanMethodA),
    CallStaticByteMethod: Some(CallStaticByteMethod),
    CallStaticByteMethodV: Some(CallStaticByteMethodV),
    CallStaticByteMethodA: Some(CallStaticByteMethodA),
    CallStaticCharMethod: Some(CallStaticCharMethod),
    CallStaticCharMethodV: Some(CallStaticCharMethodV),
    CallStaticCharMethodA: Some(CallStaticCharMethodA),
    CallStaticShortMethod: Some(CallStaticShortMethod),
    CallStaticShortMethodV: Some(CallStaticShortMethodV),
    CallStaticShortMethodA: Some(CallStaticShortMethodA),
    CallStaticIntMethod: Some(CallStaticIntMethod),
    CallStaticIntMethodV: Some(CallStaticIntMethodV),
    CallStaticIntMethodA: Some(CallStaticIntMethodA),
    CallStaticLongMethod: Some(CallStaticLongMethod),
    CallStaticLongMethodV: Some(CallStaticLongMethodV),
    CallStaticLongMethodA: Some(CallStaticLongMethodA),
    CallStaticFloatMethod: Some(CallStaticFloatMethod),
    CallStaticFloatMethodV: Some(CallStaticFloatMethodV),
    CallStaticFloatMethodA: Some(CallStaticFloatMethodA),
    CallStaticDoubleMethod: Some(CallStaticDoubleMethod),
    CallStaticDoubleMethodV: Some(CallStaticDoubleMethodV),
    CallStaticDoubleMethodA: Some(CallStaticDoubleMethodA),
    CallStaticVoidMethod: Some(CallStaticVoidMethod),
    CallStaticVoidMethodV: Some(CallStaticVoidMethodV),
    CallStaticVoidMethodA: Some(CallStaticVoidMethodA),
    GetStaticFieldID: Some(GetStaticFieldID),
    GetStaticObjectField: Some(GetStaticObjectField),
    GetStaticBooleanField: Some(GetStaticBooleanField),
    GetStaticByteField: Some(GetStaticByteField),
    GetStaticCharField: Some(GetStaticCharField),
    GetStaticShortField: Some(GetStaticShortField),
    GetStaticIntField: Some(GetStaticIntField),
    GetStaticLongField: Some(GetStaticLongField),
    GetStaticFloatField: Some(GetStaticFloatField),
    GetStaticDoubleField: Some(GetStaticDoubleField),
    SetStaticObjectField: Some(SetStaticObjectField),
    SetStaticBooleanField: Some(SetStaticBooleanField),
    SetStaticByteField: Some(SetStaticByteField),
    SetStaticCharField: Some(SetStaticCharField),
    SetStaticShortField: Some(SetStaticShortField),
    SetStaticIntField: Some(SetStaticIntField),
    SetStaticLongField: Some(SetStaticLongField),
    SetStaticFloatField: Some(SetStaticFloatField),
    SetStaticDoubleField: Some(SetStaticDoubleField),
    NewString: Some(NewString),
    GetStringLength: Some(GetStringLength),
    GetStringChars: Some(GetStringChars),
    ReleaseStringChars: Some(ReleaseStringChars),
    NewStringUTF: Some(NewStringUTF),
    GetStringUTFLength: Some(GetStringUTFLength),
    GetStringUTFChars: Some(GetStringUTFChars),
    ReleaseStringUTFChars: Some(ReleaseStringUTFChars),
    GetArrayLength: Some(GetArrayLength),
    NewObjectArray: Some(NewObjectArray),
    GetObjectArrayElement: Some(GetObjectArrayElement),
    SetObjectArrayElement: Some(SetObjectArrayElement),
    NewBooleanArray: Some(NewBooleanArray),
    NewByteArray: Some(NewByteArray),
    NewCharArray: Some(NewCharArray),
    NewShortArray: Some(NewShortArray),
    NewIntArray: Some(NewIntArray),
    NewLongArray: Some(NewLongArray),
    NewFloatArray: Some(NewFloatArray),
    NewDoubleArray: Some(NewDoubleArray),
    GetBooleanArrayElements: Some(GetBooleanArrayElements),
    GetByteArrayElements: Some(GetByteArrayElements),
    GetCharArrayElements: Some(GetCharArrayElements),
    GetShortArrayElements: Some(GetShortArrayElements),
    GetIntArrayElements: Some(GetIntArrayElements),
    GetLongArrayElements: Some(GetLongArrayElements),
    GetFloatArrayElements: Some(GetFloatArrayElements),
    GetDoubleArrayElements: Some(GetDoubleArrayElements),
    ReleaseBooleanArrayElements: Some(ReleaseBooleanArrayElements),
    ReleaseByteArrayElements: Some(ReleaseByteArrayElements),
    ReleaseCharArrayElements: Some(ReleaseCharArrayElements),
    ReleaseShortArrayElements: Some(ReleaseShortArrayElements),
    ReleaseIntArrayElements: Some(ReleaseIntArrayElements),
    ReleaseLongArrayElements: Some(ReleaseLongArrayElements),
    ReleaseFloatArrayElements: Some(ReleaseFloatArrayElements),
    ReleaseDoubleArrayElements: Some(ReleaseDoubleArrayElements),
    GetBooleanArrayRegion: Some(GetBooleanArrayRegion),
    GetByteArrayRegion: Some(GetByteArrayRegion),
    GetCharArrayRegion: Some(GetCharArrayRegion),
    GetShortArrayRegion: Some(GetShortArrayRegion),
    GetIntArrayRegion: Some(GetIntArrayRegion),
    GetLongArrayRegion: Some(GetLongArrayRegion),
    GetFloatArrayRegion: Some(GetFloatArrayRegion),
    GetDoubleArrayRegion: Some(GetDoubleArrayRegion),
    SetBooleanArrayRegion: Some(SetBooleanArrayRegion),
    SetByteArrayRegion: Some(SetByteArrayRegion),
    SetCharArrayRegion: Some(SetCharArrayRegion),
    SetShortArrayRegion: Some(SetShortArrayRegion),
    SetIntArrayRegion: Some(SetIntArrayRegion),
    SetLongArrayRegion: Some(SetLongArrayRegion),
    SetFloatArrayRegion: Some(SetFloatArrayRegion),
    SetDoubleArrayRegion: Some(SetDoubleArrayRegion),
    RegisterNatives: Some(RegisterNatives),
    UnregisterNatives: Some(UnregisterNatives),
    MonitorEnter: Some(MonitorEnter),
    MonitorExit: Some(MonitorExit),
    GetJavaVM: Some(GetJavaVM),
    GetStringRegion: Some(GetStringRegion),
    GetStringUTFRegion: Some(GetStringUTFRegion),
    GetPrimitiveArrayCritical: Some(GetPrimitiveArrayCritical),
    ReleasePrimitiveArrayCritical: Some(ReleasePrimitiveArrayCritical),
    GetStringCritical: Some(GetStringCritical),
    ReleaseStringCritical: Some(ReleaseStringCritical),
    NewWeakGlobalRef: Some(NewWeakGlobalRef),
    DeleteWeakGlobalRef: Some(DeleteWeakGlobalRef),
    ExceptionCheck: Some(ExceptionCheck),
    NewDirectByteBuffer: Some(NewDirectByteBuffer),
    GetDirectBufferAddress: Some(GetDirectBufferAddress),
    GetDirectBufferCapacity: Some(GetDirectBufferCapacity),
    GetObjectRefType: Some(GetObjectRefType),
};