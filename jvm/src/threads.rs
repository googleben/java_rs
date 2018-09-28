use std::sync::Arc;
use std::sync::RwLock;
use types::JavaType;
use types::Method;

pub struct JvmThread {
    pub pending_exception: Option<Arc<RwLock<JavaType>>>,
    pub stack: Vec<StackFrame>,
}

pub struct StackFrame {
    pub current_method: Arc<RwLock<Method>>,
    pub this: Option<Arc<RwLock<JavaType>>>,
    pub pc: u32,
    pub stack: Vec<JavaType>,
    pub locals: Vec<JavaType>,
    pub thread: Arc<RwLock<JvmThread>>,
}

impl StackFrame {
    pub fn new(current_method: Arc<RwLock<Method>>, this: Option<Arc<RwLock<JavaType>>>, thread: Arc<RwLock<JvmThread>>,
               arguments: Vec<JavaType>) -> StackFrame {
        let m = current_method.clone();
        let m = m.read().unwrap();
        let stack = Vec::with_capacity(match
            m.attributes.get(m.code_attr_index).unwrap() {
            ::java_class::attributes::Attribute::Code { max_stack, .. } => *max_stack as usize,
            _ => panic!("Incorrect code attribute")
        });
        StackFrame { current_method, this, pc: 0, stack, locals: arguments, thread }
    }
}