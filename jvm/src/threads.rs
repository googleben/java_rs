use java_class::opcodes::Opcode;
use types::JavaType;
use types::Method;
use std::sync::Arc;
use std::sync::Condvar;
use std::thread;
use std::thread::JoinHandle;
use crate::jni::JNIEnv;
use crate::jni_impl::JniRef;
use crate::types::Class;
use crate::types::ClassInitStatus;
use crate::types::RuntimeConstantPool;
use crate::types::RuntimeConstantPoolEntry;
use ::jvm;

macro_rules! exception {
    ($text:expr, $ex_type:expr) => {
        panic!("{} exception: {}", $text, $ex_type);
    };
}

#[repr(C)]
pub struct JvmThread {
    pub jni_env: JNIEnv,
    pub pending_exception: Option<JavaType>,
    pub stack: Vec<StackFrame>,
    pub jni_stack: Vec<JniStack>,
}

pub struct JniStack {
    pub frames: Vec<JniStackFrame>
}

pub struct JniStackFrame {
    pub locals: Vec<*mut JniRef>,
}

impl Drop for JniStackFrame {
    fn drop(&mut self) {
        for l in self.locals.iter() {
            unsafe {Box::from_raw(*l)};
        }
    }
}

unsafe impl Send for JvmThread {}

/// Tells the JVM whether a stack frame should be pushed or popped after an instruction was executed
enum InstructionRunInfo {
    NoChange,
    Return(Option<JavaType>),
    Call {method: &'static Method, this: Option<JavaType>, args: Vec<JavaType>},
    Branch(isize)
}

impl JvmThread {
    pub fn new(entry: &'static Method) -> JvmThread {
        JvmThread::with_args(entry, Vec::new())
    }

    pub fn with_args(entry: &'static Method, args: Vec<JavaType>) -> JvmThread {
        let mut ans = JvmThread {
            jni_env: &::jni_impl::JNI_FUNCTIONS as JNIEnv,
            pending_exception: None, stack: Vec::new(),
            jni_stack: vec!()
        };
        let frame = StackFrame::new(entry, None, args);
        ans.stack.push(frame);
        ans
    }

    pub fn call_from_jni(&mut self, method: &'static Method, this: Option<JavaType>, args: Vec<JavaType>) -> Option<JavaType> {
        self.stack.push(StackFrame::new(method, this, args));
        self.run()
    }

    pub fn start(mut self) -> JoinHandle<()> {
        thread::spawn(move || {self.run();})
    }

    fn run(&mut self) -> Option<JavaType> {
        let mut ret = InstructionRunInfo::NoChange;
        loop {
            match ret {
                InstructionRunInfo::NoChange => {
                    let frame_index = self.stack.len() - 1;
                    let frame = self.stack.get_mut(frame_index).unwrap();
                    let code = frame.current_method.code.as_ref().unwrap();
                    let cp = &frame.current_method.class.constant_pool;
                    loop {
                        ret = Self::run_inner(frame, &code.code[frame.pc], cp);
                        if let InstructionRunInfo::Branch(off) = ret {
                            if off < 0 {
                                let mut off = (-off) as usize;
                                while off != 0 {
                                    off -= code.code[frame.pc].len_bytes();
                                    frame.pc -= 1;
                                }
                            } else {
                                let mut off = off as usize;
                                while off != 0 {
                                    off += code.code[frame.pc].len_bytes();
                                    frame.pc -= 1;
                                }
                            }
                        } else if !matches!(ret, InstructionRunInfo::NoChange) {
                            frame.pc += 1;
                            break;
                        } else {
                            frame.pc += 1;
                        }
                    }
                },
                InstructionRunInfo::Call {method, this, args} => {
                    self.stack.push(StackFrame::new(method, this, args));
                    ret = InstructionRunInfo::NoChange;
                },
                InstructionRunInfo::Return(val) => {
                    self.stack.pop();
                    if self.stack.is_empty() || self.stack.last().unwrap().is_native {
                        return val;
                    }
                    if let Some(val) = val {
                        let frame_index = self.stack.len() - 1;
                        self.stack.get_mut(frame_index).unwrap().stack.push(val);
                    }
                    ret = InstructionRunInfo::NoChange;
                },
                _ => {unreachable!()}
            }
            
        }
    }

    fn run_inner(frame: &mut StackFrame, ins: &Opcode, cp: &RuntimeConstantPool) -> InstructionRunInfo {
        //TODO: exceptions
        use java_class::opcodes::Opcode::*;
        trace!("running instruction {:?}", ins);
        match ins {
            aaload | baload | saload | iaload | laload | faload | daload | caload => {
                if let JavaType::Int(index) = frame.pop() {
                    let array = frame.pop();
                    if array.is_null() {
                        exception!("", "NullPointerException");
                    }
                    if index < 0 || index >= array.array_length() {
                        exception!("", "ArrayIndexOutOfBoundsException");
                    }
                    frame.push(array.array_get(index as usize));
                } else {
                    panic!()
                }
            },
            aastore | bastore | sastore | iastore | lastore | fastore | dastore | castore => {
                let val = frame.pop();
                if let JavaType::Int(index) = frame.pop() {
                    let array = frame.pop();
                    if array.is_null() {
                        exception!("", "NullPointerException");
                    }
                    if index < 0 || index >= array.array_length() {
                        exception!("", "ArrayIndexOutOfBoundsException");
                    }
                    array.array_set(index as usize, val);
                } else {
                    panic!()
                }
            },
            aconst_null => {
                frame.push(JavaType::Null);
            },
            aload {index} | dload {index} | fload {index} | iload {index} | lload {index} => {
                frame.push(frame.locals[*index as usize].clone());
            },
            //wide for iload, lload, etc.
            wide {opcode: 0x15, index} | wide {opcode: 0x16, index} | wide {opcode: 0x17, index} | wide {opcode: 0x18, index} | wide {opcode: 0x19, index} => {
                frame.locals[*index as usize] = frame.pop();
            },
            aload_0 | dload_0 | fload_0 | iload_0 | lload_0 => {
                frame.push(frame.locals[0].clone());
            },
            aload_1 | dload_1 | fload_1 | iload_1 | lload_1 => {
                frame.push(frame.locals[1].clone());
            },
            aload_2 | dload_2 | fload_2 | iload_2 | lload_2 => {
                frame.push(frame.locals[2].clone());
            },
            aload_3 | dload_3 | fload_3 | iload_3 | lload_3 => {
                frame.push(frame.locals[3].clone());
            },
            anewarray { index } => {
                let len = frame.pop().cast_usize();
                if let RuntimeConstantPoolEntry::Class(class) = cp[*index as usize] {
                    frame.push(jvm::create_array(class, len));
                }
            },
            newarray { atype } => {
                let len = frame.pop().cast_usize();
                let class = jvm::get_class(match *atype {
                    4 => "Z",
                    5 => "C",
                    6 => "F",
                    7 => "D",
                    8 => "B",
                    9 => "S",
                    10 => "I",
                    11 => "J",
                    _ => panic!()
                }).unwrap();
                frame.push(jvm::create_array(class, len));
            },
            multianewarray { index, dimensions } => {
                let class = if let RuntimeConstantPoolEntry::Class(class) = cp[*index as usize] {
                    class
                } else {
                    panic!();
                };
                let mut lens = vec!();
                for _ in 0..*dimensions {
                    if let JavaType::Int(i) = frame.pop() {
                        lens.push(i as usize);
                    } else {
                        panic!();
                    }
                }
                lens.reverse();
                let ans = jvm::create_array(class.array_inner.unwrap(), lens[0]);
                let mut next = vec![ans.clone()];
                let mut class = class.array_inner.unwrap();
                for &len in lens.iter().skip(1) {
                    let curr = next;
                    next = Vec::with_capacity(len);
                    class = class.array_inner.unwrap();
                    let n = curr[0].array_length() as usize;
                    for c in curr {
                        for i in 0..n {
                            let tmp = jvm::create_array(class, len);
                            next.push(tmp.clone());
                            c.array_set(i, tmp);
                        }
                    }
                }
                frame.push(ans);
            },
            areturn | dreturn | freturn | ireturn | lreturn => {
                return InstructionRunInfo::Return(Some(frame.pop()))
            },
            arraylength => {
                let arr = frame.pop();
                frame.push(JavaType::Int(arr.array_length()));
            },
            astore {index} | dstore {index} | fstore {index} | istore {index} | lstore {index} => {
                frame.locals[*index as usize] = frame.pop();
            },
            //wide for istore, lstore, etc.
            wide {opcode: 0x36, index} | wide {opcode: 0x37, index} | wide {opcode: 0x38, index} | wide {opcode: 0x39, index} | wide {opcode: 0x3a, index} => {
                frame.locals[*index as usize] = frame.pop();
            },
            astore_0 | dstore_0 | fstore_0 | istore_0 | lstore_0 => {
                frame.locals[0] = frame.pop();
            },
            astore_1 | dstore_1 | fstore_1 | istore_1 | lstore_1 => {
                frame.locals[1] = frame.pop();
            },
            astore_2 | dstore_2 | fstore_2 | istore_2 | lstore_2 => {
                frame.locals[2] = frame.pop();
            }
            astore_3 | dstore_3 | fstore_3 | istore_3 | lstore_3 => {
                frame.locals[2] = frame.pop();
            },
            athrow => {
                //TODO: throw exception
            },
            bipush {val} => {
                frame.push(JavaType::Int(*val as i32));
            },
            sipush {val} => {
                frame.push(JavaType::Int(*val as i32));
            }
            breakpoint => {
                //TODO: breakpoints
            },
            checkcast {index} => {
                if let RuntimeConstantPoolEntry::Class(expected) = cp[*index as usize] {
                    let obj = frame.pop();
                    if let JavaType::Reference {class, ..} = obj {
                        if class.instanceof(expected) {
                            exception!("", "ClassCastException");
                        }
                    } else {
                        panic!()
                    }
                    frame.push(obj);
                } else {
                    panic!()
                }
            },
            d2f => {
                if let JavaType::Double(val) = frame.pop() {
                    //TODO: fp-strictness
                    frame.push(JavaType::Float(val as f32));
                }
            },
            d2i => {
                if let JavaType::Double(val) = frame.pop() {
                    frame.push(JavaType::Int(
                        if val.is_nan() { 0 }
                        else if val.is_finite() { val as i32 }
                        else if val.is_sign_negative() { i32::MIN }
                        else { i32::MAX }
                    ))
                }
            },
            d2l => {
                if let JavaType::Double(val) = frame.pop() {
                    frame.push(JavaType::Long(
                        if val.is_nan() { 0 }
                        else if val.is_finite() { val as i64 }
                        else if val.is_sign_negative() { i64::MIN }
                        else { i64::MAX }
                    ))
                }
            },
            dadd => {
                if let JavaType::Double(a) = frame.pop() {
                    if let JavaType::Double(b) = frame.pop() {
                        frame.push(JavaType::Double(a+b));
                    }
                }
            },
            dcmpg => {
                if let JavaType::Double(a) = frame.pop() {
                    if let JavaType::Double(b) = frame.pop() {
                        if a.is_nan() || b.is_nan() {
                            frame.push(JavaType::Int(1));
                        } else {
                            frame.push(JavaType::Int(if (a - b).abs() <= f64::EPSILON {0} else if a > b {1} else {-1}));
                        }
                    }
                }
            },
            dcmpl => {
                if let JavaType::Double(a) = frame.pop() {
                    if let JavaType::Double(b) = frame.pop() {
                        if a.is_nan() || b.is_nan() {
                            frame.push(JavaType::Int(-1));
                        } else {
                            frame.push(JavaType::Int(if (a - b).abs() <= f64::EPSILON {0} else if a > b {1} else {-1}));
                        }
                    }
                }
            },
            dconst_0 => {
                frame.push(JavaType::Double(0f64));
            },
            dconst_1 => {
                frame.push(JavaType::Double(1f64));
            },
            ddiv => {
                if let JavaType::Double(a) = frame.pop() {
                    if let JavaType::Double(b) = frame.pop() {
                        frame.push(JavaType::Double(a/b));
                    }
                }
            },
            dmul => {
                if let JavaType::Double(a) = frame.pop() {
                    if let JavaType::Double(b) = frame.pop() {
                        frame.push(JavaType::Double(a*b));
                    }
                }
            },
            dneg => {
                if let JavaType::Double(a) = frame.pop() {
                    frame.push(JavaType::Double(-a));
                }
            },
            drem => {
                if let JavaType::Double(a) = frame.pop() {
                    if let JavaType::Double(b) = frame.pop() {
                        frame.push(JavaType::Double(a.rem_euclid(b)));
                    }
                }
            },
            dsub => {
                if let JavaType::Double(a) = frame.pop() {
                    if let JavaType::Double(b) = frame.pop() {
                        frame.push(JavaType::Double(a-b));
                    }
                }
            },
            dup => {
                let a = frame.pop_exp();
                frame.push_exp(a.clone());
                frame.push_exp(a);
            },
            dup_x1 => {
                let a = frame.pop_exp();
                let b = frame.pop_exp();
                frame.push_exp(a.clone());
                frame.push_exp(b);
                frame.push_exp(a);
            },
            dup_x2 => {
                let a = frame.pop_exp();
                let b = frame.pop_exp();
                let c = frame.pop_exp();
                frame.push_exp(a.clone());
                frame.push_exp(c);
                frame.push_exp(b);
                frame.push_exp(a);
            },
            dup2 => {
                let a = frame.pop_exp();
                let b = frame.pop_exp();
                frame.push_exp(b.clone());
                frame.push_exp(a.clone());
                frame.push_exp(b);
                frame.push_exp(a);
            },
            dup2_x1 => {
                let a = frame.pop_exp();
                let b = frame.pop_exp();
                let c = frame.pop_exp();
                frame.push_exp(b.clone());
                frame.push_exp(a.clone());
                frame.push_exp(c);
                frame.push_exp(b);
                frame.push_exp(a);
            },
            dup2_x2 => {
                let a = frame.pop_exp();
                let b = frame.pop_exp();
                let c = frame.pop_exp();
                let d = frame.pop_exp();
                frame.push_exp(b.clone());
                frame.push_exp(a.clone());
                frame.push_exp(d);
                frame.push_exp(c);
                frame.push_exp(b);
                frame.push_exp(a);
            },
            f2d => {
                if let JavaType::Float(f) = frame.pop() {
                    //TODO: handle strictfp
                    frame.push(JavaType::Double(f as f64))
                }
            },
            f2i => {
                if let JavaType::Float(f) = frame.pop() {
                    frame.push(JavaType::Int(
                        if f.is_nan() {
                            0
                        } else if f.is_finite() {
                            f as i32
                        } else if f.is_sign_positive() {
                            i32::max_value()
                        } else {
                            i32::min_value()
                        }
                    ))
                }
            },
            f2l => {
                if let JavaType::Float(f) = frame.pop() {
                    frame.push(JavaType::Long(
                        if f.is_nan() {
                            0
                        } else if f.is_finite() {
                            f as i64
                        } else if f.is_sign_positive() {
                            i64::max_value()
                        } else {
                            i64::min_value()
                        }
                    ))
                }
            },
            fadd => {
                if let JavaType::Float(b) = frame.pop() {
                    if let JavaType::Float(a) = frame.pop() {
                        //TODO: make sure rust fp math behaves by the same rules as java expects
                        frame.push(JavaType::Float(a + b))
                    }
                }
            },
            fcmpg => {
                if let JavaType::Float(a) = frame.pop() {
                    if let JavaType::Float(b) = frame.pop() {
                        if a.is_nan() || b.is_nan() {
                            frame.push(JavaType::Int(1));
                        } else {
                            frame.push(JavaType::Int(if (a - b).abs() <= f32::EPSILON {0} else if a > b {1} else {-1}));
                        }
                    }
                }
            },
            fcmpl => {
                if let JavaType::Float(a) = frame.pop() {
                    if let JavaType::Float(b) = frame.pop() {
                        if a.is_nan() || b.is_nan() {
                            frame.push(JavaType::Int(-1));
                        } else {
                            frame.push(JavaType::Int(if (a - b).abs() <= f32::EPSILON {0} else if a > b {1} else {-1}));
                        }
                    }
                }
            },
            fconst_0 => {
                frame.push(JavaType::Float(0f32))
            },
            fconst_1 => {
                frame.push(JavaType::Float(1f32))
            },
            fconst_2 => {
                frame.push(JavaType::Float(2f32))
            },
            fdiv => {
                if let JavaType::Float(b) = frame.pop() {
                    if let JavaType::Float(a) = frame.pop() {
                        frame.push(JavaType::Float(a / b))
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            fmul => {
                if let JavaType::Float(b) = frame.pop() {
                    if let JavaType::Float(a) = frame.pop() {
                        frame.push(JavaType::Float(a * b))
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            fneg => {
                if let JavaType::Float(val) = frame.pop() {
                    frame.push(JavaType::Float(-val))
                } else {
                    panic!()
                }
            },
            frem => {
                if let JavaType::Float(b) = frame.pop() {
                    if let JavaType::Float(a) = frame.pop() {
                        frame.push(JavaType::Float(a % b))
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            fsub => {
                if let JavaType::Float(b) = frame.pop() {
                    if let JavaType::Float(a) = frame.pop() {
                        frame.push(JavaType::Float(a - b))
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            getfield {index} => {
                let field_desc = &cp[*index as usize];
                if let RuntimeConstantPoolEntry::Fieldref {name, ..} = field_desc {
                    let obj = frame.pop();
                    if obj.is_null() {
                        exception!("", "NullPointerException");
                    }
                    frame.push(obj.get_field(name));
                } else {
                    panic!()
                }
            },
            getstatic {index} => {
                let field_desc = &cp[*index as usize];
                if let RuntimeConstantPoolEntry::Fieldref {class, name, ..} = field_desc {
                    ensure_class_init(class);
                    frame.push(class.fields.get(name).unwrap().value.read().unwrap().clone())
                }
            },
            goto {branch} => {
                return InstructionRunInfo::Branch(*branch as isize);
            },
            goto_w {branch} => {
                return InstructionRunInfo::Branch(*branch as isize);
            },
            i2b => {
                if let JavaType::Int(val) = frame.pop() {
                    frame.push(JavaType::Byte(val as i8));
                } else {
                    panic!();
                }
            },
            i2c => {
                if let JavaType::Int(val) = frame.pop() {
                    frame.push(JavaType::Char(val as u16));
                } else {
                    panic!();
                }
            },
            i2d => {
                if let JavaType::Int(val) = frame.pop() {
                    frame.push(JavaType::Double(val as f64));
                } else {
                    panic!();
                }
            },
            i2f => {
                if let JavaType::Int(val) = frame.pop() {
                    frame.push(JavaType::Float(val as f32));
                } else {
                    panic!();
                }
            },
            i2l => {
                if let JavaType::Int(val) = frame.pop() {
                    frame.push(JavaType::Long(val as i64));
                } else {
                    panic!();
                }
            },
            i2s => {
                if let JavaType::Int(val) = frame.pop() {
                    frame.push(JavaType::Short(val as i16));
                } else {
                    panic!();
                }
            },
            iadd => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a + b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            iand => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a & b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            iconst_m1 => {
                frame.push(JavaType::Int(-1));
            },
            iconst_0 => {
                frame.push(JavaType::Int(0));
            },
            iconst_1 => {
                frame.push(JavaType::Int(1));
            },
            iconst_2 => {
                frame.push(JavaType::Int(2));
            },
            iconst_3 => {
                frame.push(JavaType::Int(3));
            },
            iconst_4 => {
                frame.push(JavaType::Int(4));
            },
            iconst_5 => {
                frame.push(JavaType::Int(5));
            },
            idiv => {
                if let JavaType::Int(b) = frame.pop() {
                    if b == 0 {
                        exception!("Attempt to divide by 0", "ArithmeticException");
                    }
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a / b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            if_acmpeq {branch} => {
                if let JavaType::Reference {val: b, ..} = frame.pop() {
                    if let JavaType::Reference {val: a, ..} = frame.pop() {
                        if Arc::ptr_eq(&a, &b) {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            if_acmpne {branch} => {
                if let JavaType::Reference {val: b, ..} = frame.pop() {
                    if let JavaType::Reference {val: a, ..} = frame.pop() {
                        if !Arc::ptr_eq(&a, &b) {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            if_icmpeq {branch} => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        if a == b {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            if_icmpne {branch} => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        if a != b {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            if_icmplt {branch} => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        if a < b {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            if_icmple {branch} => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        if a <= b {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            if_icmpgt {branch} => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        if a > b {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            if_icmpge {branch} => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        if a >= b {
                            return InstructionRunInfo::Branch(*branch as isize);
                        }
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            ifeq {branch} => {
                if let JavaType::Int(val) = frame.pop() {
                    if val == 0 {
                        return InstructionRunInfo::Branch(*branch as isize);
                    }
                } else {
                    panic!()
                }
            },
            ifne {branch} => {
                if let JavaType::Int(val) = frame.pop() {
                    if val != 0 {
                        return InstructionRunInfo::Branch(*branch as isize);
                    }
                } else {
                    panic!()
                }
            },
            iflt {branch} => {
                if let JavaType::Int(val) = frame.pop() {
                    if val < 0 {
                        return InstructionRunInfo::Branch(*branch as isize);
                    }
                } else {
                    panic!()
                }
            },
            ifle {branch} => {
                if let JavaType::Int(val) = frame.pop() {
                    if val <= 0 {
                        return InstructionRunInfo::Branch(*branch as isize);
                    }
                } else {
                    panic!()
                }
            },
            ifgt {branch} => {
                if let JavaType::Int(val) = frame.pop() {
                    if val > 0 {
                        return InstructionRunInfo::Branch(*branch as isize);
                    }
                } else {
                    panic!()
                }
            },
            ifge {branch} => {
                if let JavaType::Int(val) = frame.pop() {
                    if val >= 0 {
                        return InstructionRunInfo::Branch(*branch as isize);
                    }
                } else {
                    panic!()
                }
            },
            ifnonnull {branch} => {
                if !frame.pop().is_null() {
                    return InstructionRunInfo::Branch(*branch as isize);
                }
            },
            ifnull {branch} => {
                if frame.pop().is_null() {
                    return InstructionRunInfo::Branch(*branch as isize);
                }
            },
            iinc {index, const_} => {
                if let JavaType::Int(val) = frame.locals[*index as usize] {
                    frame.locals[*index as usize] = JavaType::Int(val.wrapping_add(*const_ as i32));
                } else {
                    panic!()
                }
            },
            wide_iinc { index, const_ } => {
                if let JavaType::Int(val) = frame.locals[*index as usize] {
                    frame.locals[*index as usize] = JavaType::Int(val.wrapping_add(*const_ as i32));
                } else {
                    panic!()
                }
            }
            imul => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a.wrapping_mul(b)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            ineg => {
                if let JavaType::Int(val) = frame.pop() {
                    frame.push(JavaType::Int(val.wrapping_neg()));
                } else {
                    panic!();
                }
            },
            instanceof {index} => {
                if let RuntimeConstantPoolEntry::Class(expected) = cp[*index as usize] {
                    let obj = frame.pop();
                    if let JavaType::Reference {class, ..} = obj {
                        if class.instanceof(expected) {
                            frame.push(JavaType::Int(0));
                        } else {
                            frame.push(JavaType::Int(1));
                        }
                    } else if obj.is_null() {
                        frame.push(JavaType::Int(0));
                    } else {
                        panic!()
                    }
                } else {
                    panic!()
                }
            },
            invokedynamic { index: _index } => {
                //TODO: invokedynamic
                todo!();
            },
            invokeinterface { index, count } => {
                let (interface, name, descriptor) = {
                    if let RuntimeConstantPoolEntry::InterfaceMethodref {class, name, descriptor} = &cp[*index as usize] {
                        (*class, name, descriptor)
                    } else {
                        panic!()
                    }
                };
                let method = interface.resolve_interface_method(name, descriptor);
                let method = if let Ok(m) = method {
                    m
                } else {
                    exception!("", method.unwrap_err());
                };
                if method.name.ends_with("/<clinit>") || method.name.ends_with("/<init>") {
                    panic!("Attempt to invokeinterface on an initialization method");
                }
        
                let mut args = Vec::with_capacity(*count as usize);
                for _ in 0..(*count) {
                    args.push(frame.pop_exp());
                }
                args.reverse();
                let obj = frame.pop();
                if obj.is_null() {
                    exception!("", "NullPointerException");
                }

                let class = if let JavaType::Reference {class, ..} = obj {
                    class
                } else {
                    panic!()
                };
                if !class.implements(interface) {
                    exception!("", "IncompatibleClassChangeError");
                }
                let method = class.resolve_method_invokeinterface(name, descriptor);
                let method = if let Ok(m) = method {
                    m
                } else {
                    exception!("", method.unwrap_err());
                };
                use java_class::methods::AccessFlags;
                if method.access_flags & AccessFlags::Native as u16 != 0 {
                    //TODO: native methods
                    todo!("Native methods are unimplemented");
                } else {
                    return InstructionRunInfo::Call {method, this: Some(obj), args}
                }
            },
            invokespecial {index} => {
                //TODO: access checks
                let (class, name, descriptor, method) = match &cp[*index as usize] {
                    RuntimeConstantPoolEntry::InterfaceMethodref {class, name, descriptor} => {
                        (*class, name, descriptor, class.resolve_interface_method(name, descriptor))
                    },
                    RuntimeConstantPoolEntry::Methodref {class, name, descriptor} => {
                        (*class, name, descriptor, class.resolve_method(name, descriptor))
                    }
                    _ => {
                        panic!()
                    }
                };
                let method = if let Ok(m) = method {
                    m
                } else {
                    exception!("", method.unwrap_err());
                };
                use java_class::methods::AccessFlags;
                //If the resolved method is protected, and it is a member of a superclass 
                //of the current class, and the method is not declared in the same 
                //run-time package (§5.3) as the current class, then the class of objectref 
                //must be either the current class or a subclass of the current class.
                let required_superclass = if method.access_flags & AccessFlags::Protected as u16 != 0 {
                    if method.class.get_package() != class.get_package() {
                        Some(class)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let method = class.resolve_method_invokespecial(name, descriptor);
                let method = if let Ok(m) = method {
                    m
                } else {
                    exception!("", method.unwrap_err());
                };
                if method.name.ends_with("/<init>") && method.class != class {
                    exception!("", "NoSuchMethodError");
                }
                if method.access_flags & AccessFlags::Static as u16 != 0 {
                    exception!("", "IncompatibleClassChangeError");
                }
                if method.is_abstract() {
                    exception!("", "AbstractMethodError");
                }
                let count = method.parameters.len();
                let mut args = Vec::with_capacity(count);
                for _ in 0..(count) {
                    let tmp = frame.pop();
                    if matches!(tmp, JavaType::Double(_) | JavaType::Long(_)) {
                        args.push(JavaType::Null);
                    }
                    args.push(tmp);
                }
                args.reverse();
                let obj = frame.pop();
                if obj.is_null() {
                    exception!("", "NullPointerException");
                }
                if let JavaType::Reference {class, ..} = obj {
                    if let Some(required_superclass) = required_superclass {
                        let mut ok = false;
                        let mut curr = Some(class);
                        while let Some(class) = curr {
                            if class == required_superclass {
                                ok = true;
                                break;
                            }
                            curr = class.super_class;
                        }
                        if !ok {
                            exception!("", "IllegalAccessError");
                        }
                    }
                } else {
                    panic!();
                }
                //TODO: synchronization
                
                if method.is_native() {
                    //TODO: native methods
                    todo!("Native methods are unimplemented");
                }
                return InstructionRunInfo::Call {method, this: Some(obj), args};
            },
            invokestatic {index} => {
                let (class, name, descriptor) = match &cp[*index as usize] {
                    RuntimeConstantPoolEntry::InterfaceMethodref {class, name, descriptor} => {
                        (*class, name, descriptor)
                    },
                    RuntimeConstantPoolEntry::Methodref {class, name, descriptor} => {
                        (*class, name, descriptor)
                    }
                    _ => {
                        panic!()
                    }
                };
                ensure_class_init(class);
                //TODO: On successful resolution of the method, the class or interface that declared the resolved method is initialized (§5.5) if that class or interface has not already been initialized.
                let m = class.resolve_static_method(name, descriptor);
                let m = if let Ok(m) = m {
                    m
                } else {
                    exception!("", m.unwrap_err());
                };
                if m.access_flags & java_class::methods::AccessFlags::Static as u16 == 0 {
                    exception!("Attempted to invokestatic on instance method", "IncompatibleClassChangeError");
                }
                let count = m.parameters.len();
                let mut args = Vec::with_capacity(count);
                for _ in 0..(count) {
                    let tmp = frame.pop();
                    if matches!(tmp, JavaType::Double(_) | JavaType::Long(_)) {
                        args.push(JavaType::Null);
                    }
                    args.push(tmp);
                }
                args.reverse();
                //TODO: synchronization
                
                if m.is_native() {
                    //TODO: native methods
                    todo!("Native methods are unimplemented");
                }
                return InstructionRunInfo::Call {method: m, this: None, args};
            },
            invokevirtual { index } => {
                let (class, name, descriptor) = {
                    if let RuntimeConstantPoolEntry::Methodref {class, name, descriptor} = &cp[*index as usize] {
                        (*class, name, descriptor)
                    } else {
                        panic!()
                    }
                };
                
                let method = class.resolve_method(name, descriptor);
                let method = if let Ok(m) = method {
                    m
                } else {
                    exception!("", method.unwrap_err());
                };
                if method.name.ends_with("/<clinit>") || method.name.ends_with("/<init>") {
                    panic!();
                }
                let mut required_superclass = None;
                //security/visibility check
                if method.is_protected() && method.class != class && method.class.get_package() != class.get_package() {
                    required_superclass = Some(class);
                }
                //TODO: signature polymorphic methods (invokedynamic)
                let method = class.resolve_method_invokevirtual(name, descriptor, method);
                let method = if let Ok(m) = method {
                    m
                } else {
                    exception!("", method.unwrap_err());
                };
                //TODO: synchronized
                if method.is_native() {
                    //TODO: native methods
                    todo!("Native methods are unimplemented");
                }
                let count = method.parameters.len();
                let mut args = Vec::with_capacity(count);
                for _ in 0..(count) {
                    let tmp = frame.pop();
                    if matches!(tmp, JavaType::Double(_) | JavaType::Long(_)) {
                        args.push(JavaType::Null);
                    }
                    args.push(tmp);
                }
                args.reverse();
                let this = frame.pop();
                if let JavaType::Reference {class, ..} = this {
                    if let Some(required_superclass) = required_superclass {
                        let mut ok = false;
                        let mut curr = Some(class);
                        while let Some(class) = curr {
                            if class == required_superclass {
                                ok = true;
                                break;
                            }
                            curr = class.super_class;
                        }
                        if !ok {
                            exception!("", "IllegalAccessError");
                        }
                    }
                } else {
                    panic!();
                }
                //TODO: synchronization
                
                return InstructionRunInfo::Call {method, this: None, args};
            },
            ior => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a | b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            irem => {
                if let JavaType::Int(b) = frame.pop() {
                    if b == 0 {
                        exception!("Attempt to divide by 0", "ArithmeticException");
                    }
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a | b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            ishl => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a << (b & 0x1f)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            ishr => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a >> b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            isub => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a.wrapping_sub(b)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            iushr => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int((a >> b).abs()));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            ixor => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a ^ b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            jsr {..} => {
                unimplemented!()
            },
            jsr_w {..} => {
                unimplemented!()
            },
            l2d => {
                if let JavaType::Long(val) = frame.pop() {
                    frame.push(JavaType::Double(val as f64));
                } else {
                    panic!();
                }
            },
            l2f => {
                if let JavaType::Long(val) = frame.pop() {
                    frame.push(JavaType::Float(val as f32));
                } else {
                    panic!();
                }
            },
            l2i => {
                if let JavaType::Long(val) = frame.pop() {
                    frame.push(JavaType::Int(val as i32));
                } else {
                    panic!();
                }
            },
            ladd => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a + b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            land => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a & b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lcmp => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(match a.cmp(&b) {
                            std::cmp::Ordering::Less => -1,
                            std::cmp::Ordering::Equal => 0,
                            std::cmp::Ordering::Greater => 1,
                        }));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lconst_0 => {
                frame.push(JavaType::Long(0));
            },
            lconst_1 => {
                frame.push(JavaType::Long(1));
            }
            ldc {index} => {
                match &cp[*index as usize] {
                    RuntimeConstantPoolEntry::Integer(val) => {
                        frame.push(JavaType::Int(*val));
                    },
                    RuntimeConstantPoolEntry::Float(val) => {
                        frame.push(JavaType::Float(*val));
                    },
                    RuntimeConstantPoolEntry::String(s) => {
                        frame.push(s.clone());
                    },
                    RuntimeConstantPoolEntry::Class(class) => {
                        frame.push(class.get_class_obj());
                    },
                    RuntimeConstantPoolEntry::Methodref {..} => {
                        todo!("invokedynamic");
                    },
                    _ => {
                        panic!()
                    }
                }
            },
            ldc_w {index} => {
                match &cp[*index as usize] {
                    RuntimeConstantPoolEntry::Integer(val) => {
                        frame.push(JavaType::Int(*val));
                    },
                    RuntimeConstantPoolEntry::Float(val) => {
                        frame.push(JavaType::Float(*val));
                    },
                    RuntimeConstantPoolEntry::String(s) => {
                        frame.push(s.clone());
                    },
                    RuntimeConstantPoolEntry::Class(class) => {
                        frame.push(class.get_class_obj());
                    },
                    RuntimeConstantPoolEntry::Methodref {..} => {
                        todo!("invokedynamic");
                    },
                    _ => {
                        panic!()
                    }
                }
            },
            ldc2_w { index } => {
                match &cp[*index as usize] {
                    RuntimeConstantPoolEntry::Long(val) => {
                        frame.push(JavaType::Long(*val));
                    },
                    RuntimeConstantPoolEntry::Double(val) => {
                        frame.push(JavaType::Double(*val));
                    },
                    _ => {
                        panic!();
                    }
                }
            },
            ldiv => {
                if let JavaType::Int(b) = frame.pop() {
                    if b == 0 {
                        exception!("Attempt to divide by 0", "ArithmeticException");
                    }
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a.wrapping_div(b)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lmul => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a.wrapping_mul(b)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lneg => {
                if let JavaType::Int(a) = frame.pop() {
                    frame.push(JavaType::Int(-a));
                } else {
                    panic!();
                }
            },
            lookupswitch { default, match_offset_pairs, .. } => {
                let key = if let JavaType::Int(i) = frame.pop() {
                    i
                } else {
                    panic!();
                };
                for &(k, v) in match_offset_pairs {
                    if k == key {
                        return InstructionRunInfo::Branch(v as isize);
                    }
                }
                return InstructionRunInfo::Branch(*default as isize);
            },
            lor => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a | b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lrem => {
                if let JavaType::Int(b) = frame.pop() {
                    if b == 0 {
                        exception!("Attempt to divide by 0", "ArithmeticException");
                    }
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a.wrapping_rem(b)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lshl => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a << (b & 0x3f)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lshr => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a >> b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lsub => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a.wrapping_sub(b)));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lushr => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int((a >> b).abs()));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            lxor => {
                if let JavaType::Int(b) = frame.pop() {
                    if let JavaType::Int(a) = frame.pop() {
                        frame.push(JavaType::Int(a ^ b));
                    } else {
                        panic!();
                    }
                } else {
                    panic!();
                }
            },
            monitorenter => {
                todo!();
            },
            monitorexit => {
                todo!();
            },
            new {index} => {
                let class = if let RuntimeConstantPoolEntry::Class(c) = &cp[*index as usize] {
                    c
                } else {
                    panic!();
                };
                ensure_class_init(class);
                frame.push(class.instantiate());
            },
            nop => {},
            pop => {
                frame.pop();
            },
            pop2 => {
                frame.pop();
            },
            putfield { index } => {
                let name = if let RuntimeConstantPoolEntry::Fieldref {name, ..} = &cp[*index as usize] {
                    name
                } else {
                    panic!();
                };
                let val = frame.pop();
                let obj = frame.pop();
                if obj.is_null() {
                    exception!("", "NullPointerException");
                }
                obj.set_field(name, val);
            },
            putstatic { index } => {
                let (&class, name) = if let RuntimeConstantPoolEntry::Fieldref {class, name, ..} = &cp[*index as usize] {
                    (class, name)
                } else {
                    panic!();
                };
                ensure_class_init(class);
                let val = frame.pop();
                //TODO: initialize static class
                let mut f = class.fields.get(name).unwrap().value.write().unwrap();
                *f = val;
            },
            ret {..} => {
                unimplemented!();
            },
            //wide for ret
            wide {opcode: 0xa9, ..} => {
                unimplemented!();
            }
            return_ => {
                return InstructionRunInfo::Return(None);
            },
            swap => {
                let tmp = frame.pop();
                let tmp2 = frame.pop();
                frame.push(tmp);
                frame.push(tmp2);
            },
            tableswitch { default, low, high, jump_offsets, .. } => {
                let index = if let JavaType::Int(i) = frame.pop() {
                    i
                } else {
                    panic!();
                };
                if index < *low || index > *high {
                    return InstructionRunInfo::Branch(*default as isize);
                }
                let index = (index - low) as usize;
                return InstructionRunInfo::Branch(jump_offsets[index] as isize);
            },
            wide {opcode, ..} => {
                panic!("wide with invalid opcode {}", opcode);
            }
            reserved => {
                panic!("Attempt to execute reserved instruction");
            },
            impdep1 => {},
            impdep2 => {}
        }
        InstructionRunInfo::NoChange
    }

    pub fn create_jni_local(&mut self, val: JavaType) -> *mut JniRef {
        let frame = self.jni_stack.last_mut().unwrap().frames.last_mut().unwrap();
        let ans = JniRef::new_local(val);
        frame.locals.push(ans);
        ans
    }

    pub fn delete_jni_local(&mut self, val: *mut JniRef) {
        let frames = &mut self.jni_stack.last_mut().unwrap().frames;
        for f in frames.iter_mut().rev() {
            let mut ind = None;
            for (i, local) in f.locals.iter().enumerate() {
                if *local == val {
                    ind = Some(i);
                    break;
                }
            }
            if let Some(i) = ind {
                let val = f.locals.swap_remove(i);
                unsafe {JniRef::delete(val)};
            }
        }
    }

    pub fn push_jni_frame(&mut self) {
        self.jni_stack.last_mut().unwrap().frames.push(JniStackFrame {locals: vec!()});
    }

    pub fn pop_jni_frame(&mut self) {
        self.jni_stack.last_mut().unwrap().frames.pop().unwrap();
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn create_jni_global(&mut self, val: JavaType) -> *mut JniRef {
        JniRef::new_global(val)
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn delete_jni_global(&mut self, val: *mut JniRef) {
        JniRef::delete(val)
    }

}

pub fn ensure_class_init(class: &'static Class) {
    let id = thread::current().id();
    let mut cv;
    let mut lock = class.is_initialized.lock().unwrap();
    loop {
            
        match &*lock {
            ClassInitStatus::Initialized => {
                return;
            },
            ClassInitStatus::Initializing(id2, cv2) => {
                if id == *id2 {
                    return;
                }
                cv = cv2.clone();
                
            },
            _ => {
                cv = Arc::new(Condvar::new());
                *lock = ClassInitStatus::Initializing(id, cv.clone());
                break;
            }
        }
        lock = cv.wait(lock).unwrap();
    }
    drop(lock);
    debug!("Running static initialization for {}", class.name);
    for f in class.fields.values() {
        if f.is_final() && f.is_static() {
            for a in &f.attributes {
                if let java_class::attributes::Attribute::ConstantValue {constantvalue_index} = a {
                    let constant = &class.constant_pool[*constantvalue_index as usize];
                    let constant = match constant {
                        &RuntimeConstantPoolEntry::Double(d) => JavaType::Double(d),
                        &RuntimeConstantPoolEntry::Float(f) => JavaType::Float(f),
                        &RuntimeConstantPoolEntry::Integer(i) => JavaType::Int(i),
                        &RuntimeConstantPoolEntry::Long(l) => JavaType::Long(l),
                        RuntimeConstantPoolEntry::String(s) => s.clone(),
                        _ => panic!()
                    };
                    let mut v = f.value.write().unwrap();
                    *v = constant;
                    break;
                }
            }
        }
    }
    if !class.is_interface() && class.super_class.is_some() {
        ensure_class_init(class.super_class.unwrap());
    }
    for interface in &class.interfaces {
        ensure_class_init(interface);
    }
    let clinit = class.methods.get("<clinit>()V");
    if let Some(&clinit) = clinit {
        debug!("Running clinit for {}", class.name);
        JvmThread::new(clinit).run();
    }
    *class.is_initialized.lock().unwrap() = ClassInitStatus::Initialized;
    cv.notify_all();
    debug!("Done running static initialization for {}", class.name);
}

pub struct StackFrame {
    pub current_method: &'static Method,
    pub this: Option<JavaType>,
    pub pc: usize,
    pub stack: Vec<JavaType>,
    pub locals: Vec<JavaType>,
    pub is_native: bool
}

impl StackFrame {
    pub fn new(current_method: &'static Method, this: Option<JavaType>,
               arguments: Vec<JavaType>) -> StackFrame {
        let stack = Vec::with_capacity(current_method.code.as_ref().unwrap().max_stack);
        //TODO: Any argument value that is of a floating-point type undergoes value set conversion (§2.8.3) prior to being stored in a local variable.
        StackFrame { current_method, this, pc: 0, stack, locals: arguments, is_native: false }
    }

    /// pop the value on top, and handle popping a dummy value in the case of long and doubles
    pub fn pop(&mut self) -> JavaType {
        let val = self.stack.pop().unwrap();
        match val {
            JavaType::Double(..) | JavaType::Long(..) => {
                //dummy slot for longs and doubles
                self.stack.pop();
            },
            _ => {}
        };
        val
    }

    /// push the value, and handle pushing a dummy value in the case of long and doubles
    pub fn push(&mut self, val: JavaType) {
        match val {
            JavaType::Double(..) | JavaType::Long(..) => {
                //dummy slot for longs and doubles
                self.stack.push(JavaType::Null);
            },
            _ => {}
        };
        self.stack.push(val);
    }

    /// explicit pop does not handle double and long values requiring two slots
    pub fn pop_exp(&mut self) -> JavaType {
        self.stack.pop().unwrap()
    }

    /// explicit push does not handle double and long values requiring two slots
    pub fn push_exp(&mut self, val: JavaType) {
        self.stack.push(val);
    }
}