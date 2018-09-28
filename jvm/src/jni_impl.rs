use jni::*;

unsafe extern "C" fn GetVersion(env: *mut JNIEnv) -> jint {
    return 0x00010008;
}

unsafe extern "C" fn DefineClass(env: *mut JNIEnv, name: *const ::std::os::raw::c_char, loader: jobject, buf: *const jbyte, len: jsize) -> jclass {
    //TODO
    return &mut _jobject { _unused: [] } as jclass;
}

unsafe extern "C" fn FindClass(env: *mut JNIEnv, name: *const ::std::os::raw::c_char) -> jclass {
    //TODO
    return &mut _jobject { _unused: [] } as jclass;
}