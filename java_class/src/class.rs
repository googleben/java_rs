use methods::MethodInfo;
use fields::FieldInfo;
use cp::ConstantPool;
use cp_info::*;
use cp_info::CPInfo::*;
use std::io;
use std::io::prelude::*;
use std::fs::File;
use attributes::*;
use bytecode_tools::*;
use std::string::String;
use std::str;
use attributes::Attribute::*;

macro_rules! build_u16 {
    ($byte_1:expr, $byte_2:expr) => {
        (($byte_1 as u16) << 8) | ($byte_2 as u16)
    };
}

macro_rules! build_u32 {
    ($byte_1:expr, $byte_2:expr, $byte_3:expr, $byte_4:expr) => {
        (($byte_1 as u32) << 24) + (($byte_2 as u32) << 16) + (($byte_3 as u32) << 8) + ($byte_4 as u32)
    };
}

pub enum AccessFlags {
    Public     = 0x0001,
    Final      = 0x0010,
    Super      = 0x0020,
    Interface  = 0x0200,
    Abstract   = 0x0400,
    Synthetic  = 0x1000,
    Annotation = 0x2000,
    Enum       = 0x4000
}

#[derive(Debug)]
pub struct JavaClass {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
    pub access_flags: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub attributes: Vec<Attribute>
}

impl JavaClass {
    pub fn new(file_name: &str) -> io::Result<JavaClass> {
        let mut r = JavaClassReader::new(file_name)?;
        let magic = r.next32()?;
        if magic != 0xCAFEBABE { return malformed() }
        let minor_version = r.next16()?;
        let major_version = r.next16()?;
        let cp_count = r.next16()?;
        let mut cp_vec: Vec<CPInfo> = vec!();
        for i in 0..cp_count-1 {
            let tag = r.next()?;
            if tag < 1 || tag > 18 || tag == 2 || tag == 13 || tag == 14 || tag == 17 { return malformed(); }
            let x: CPInfo = match tag {
                7 => Class {name_index: r.next16()? },
                9 => Fieldref { class_index: r.next16()?, name_and_type_index: r.next16()? },
                10 => Methodref { class_index: r.next16()?, name_and_type_index: r.next16()? },
                11 => InterfaceMethodref { class_index: r.next16()?, name_and_type_index: r.next16()? },
                8 => CPInfo::String { string_index: r.next16()? },
                3 => Integer { bytes: r.next32()? },
                4 => Float { bytes: r.next32()? },
                5 => Long { bytes: r.next64()? },
                6 => Double { bytes: r.next64()? },
                12 => NameAndType { name_index: r.next16()?, descriptor_index: r.next16()? },
                1 => {
                    let length = r.next16()?;
                    let mut bytes: Vec<u8> = vec!();
                    for j in 0..length {
                        bytes.insert(j as usize, r.next()?);
                    }
                    Utf8 { length, bytes }
                }
                15 => MethodHandle { reference_kind: r.next()?, reference_index: r.next16()? },
                16 => MethodType {descriptor_index: r.next16()? },
                18 => InvokeDynamic { bootstrap_method_attr_index: r.next16()?, name_and_type_index: r.next16()? },
                _ => panic!("Unreachable code, wildcard case reached in exhaustive match!") //unreachable
            };
            cp_vec.insert(i as usize, x);
        };
        let cp = match ConstantPool::new_with_info(cp_vec) {
            Ok(a) => a,
            Err(a) => return Err(io::Error::new(
                io::ErrorKind::InvalidData, 
                a.into_iter().fold(String::from(""), |acc, x| acc + "\n" + &x)))
        };
        let access_flags = r.next16()?;
        let this_class = r.next16()?;
        let super_class = r.next16()?;
        let interfaces_count = r.next16()?;
        let mut interfaces = vec!();
        for _ in 0..interfaces_count {
            interfaces.push(r.next16()?);
        }
        let fields_count = r.next16()?;
        let mut fields = Vec::with_capacity(fields_count as usize);
        for _i in 0..fields_count {
            fields.push(FieldInfo {
                access_flags: r.next16()?,
                name_index: r.next16()?,
                descriptor_index: r.next16()?,
                attributes: {
                    let x = read_attributes(&mut r, &cp);
                    match x {
                        Ok(a) => a,
                        Err(s) => return Err(io::Error::new(io::ErrorKind::InvalidData, s))
                    }
                }
            })
        }
        let methods_count = r.next16()?;
        let mut methods = Vec::with_capacity(methods_count as usize);
        for _i in 0..methods_count {
            methods.push(MethodInfo {
                access_flags: r.next16()?,
                name_index: r.next16()?,
                descriptor_index: r.next16()?,
                attributes: {
                    let x = read_attributes(&mut r, &cp);
                    match x {
                        Ok(a) => a,
                        Err(s) => return Err(io::Error::new(io::ErrorKind::InvalidData, s))
                    }
                }
            })
        }
        let attributes = {
            let x = read_attributes(&mut r, &cp);
            match x {
                Ok(a) => a,
                Err(s) => return Err(io::Error::new(io::ErrorKind::InvalidData, s))
            }
        };
        Ok(JavaClass {
            minor_version,
            major_version,
            constant_pool: cp,
            access_flags,
            this_class,
            super_class,
            interfaces,
            fields,
            methods,
            attributes
        })
    }

    pub fn get_name(self) -> String {
        match self.constant_pool[self.this_class] {
            Class {name_index} => {
                match &self.constant_pool[name_index] {
                    Utf8 { bytes, ..} => {
                        str::from_utf8(&bytes).unwrap().to_owned()
                    },
                    _ => "Class Pool index did not point to Utf8".to_owned()
                }
            },
            _ => "Class Pool index did not point to Utf8".to_owned()
        }
    }
}

fn read_attributes(r: &mut JavaClassReader, cp: &ConstantPool) -> Result<Vec<Attribute>, &'static str> {
    let num = r.next16().or(Err("read failure"))?;
    let mut ans = Vec::with_capacity(num as usize);
    for _i in 0..num {
        let name_index = r.next16().or(Err("read failure"))?;
        let attribute_length = r.next32().or(Err("read failure"))?;
        let name = match &cp[name_index] {
            Utf8 { length: _, bytes } => str::from_utf8(bytes).or(Err("bad utf8"))?,
            _ => return Err("incorrect cp type")
        };
        ans.push(match name {
            "ConstantValue" => ConstantValue { constantvalue_index: r.next16().or(Err("read failure"))? },
            "Code" => Code {
                max_stack: r.next16().or(Err("read failure"))?,
                max_locals: r.next16().or(Err("read failure"))?,
                code: {
                    let code_len = r.next32().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(code_len as usize);
                    let start = r.dist();
                    while r.dist()-start<code_len {
                        ans.push(to_opcode(r).or(Err("bad bytecode read"))?)
                    }
                    ans
                },
                exception_table: {
                    let len = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(ExceptionTableEntry {
                            start_pc: r.next16().or(Err("read failure"))?,
                            end_pc: r.next16().or(Err("read failure"))?,
                            handler_pc: r.next16().or(Err("read failure"))?,
                            catch_type: r.next16().or(Err("read failure"))?
                        });
                    }
                    ans
                },
                attributes: {
                    read_attributes(r, &cp)?
                }
            },
            "StackMapTable" => StackMapTable {
                entries: {
                    let num = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        let tag = r.next().or(Err("read failure"))?;
                        ans.push(match tag {
                            0...63 => StackMapFrame::SameFrame,
                            64...127 => StackMapFrame::SameLocals1Item {
                                stack: read_verification_type_info(r)?
                            },
                            247 => StackMapFrame::SameLocals1ItemExtended {
                                offset_delta: r.next16().or(Err("read failure"))?,
                                stack: read_verification_type_info(r)?
                            },
                            248...250 => StackMapFrame::ChopFrame {
                                offset_delta: r.next16().or(Err("read failure"))?
                            },
                            251 => StackMapFrame::SameFrameExtended {
                                offset_delta: r.next16().or(Err("read failure"))?
                            },
                            252...254 => StackMapFrame::AppendFrame {
                                offset_delta: r.next16().or(Err("read failure"))?,
                                locals: {
                                    let mut ans = vec!();
                                    for _i in 0..(tag-251) {
                                        ans.push(read_verification_type_info(r)?);
                                    }
                                    ans
                                }
                            },
                            255 => StackMapFrame::FullFrame {
                                offset_delta: r.next16().or(Err("read failure"))?,
                                locals: {
                                    let mut ans = vec!();
                                    for _i in 0..r.next16().or(Err("read failure"))? {
                                        ans.push(read_verification_type_info(r)?);
                                    }
                                    ans
                                },
                                stack: {
                                    let mut ans = vec!();
                                    for _i in 0..r.next16().or(Err("read failure"))? {
                                        ans.push(read_verification_type_info(r)?);
                                    }
                                    ans
                                }
                            },
                            _ => return Err("invalid stackmapframe tag")
                        });
                    }
                    ans
                }
            },
            "Exceptions" => Exceptions {
                exception_index_table: {
                    let num = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(r.next16().or(Err("read failure"))?);
                    }
                    ans
                }
            },
            "InnerClasses" => InnerClasses {
                classes: {
                    let num = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(InnerClassInfo {
                            inner_class_info_index: r.next16().or(Err("read failure"))?,
                            outer_class_info_index: r.next16().or(Err("read failure"))?,
                            inner_name_index: r.next16().or(Err("read failure"))?,
                            inner_class_access_flags: r.next16().or(Err("read failure"))?
                        })
                    }
                    ans
                }
            },
            "EnclosingMethod" => EnclosingMethod {
                class_index: r.next16().or(Err("read failure"))?,
                method_index: r.next16().or(Err("read failure"))?
            },
            "Synthetic" => Synthetic,
            "Signature" => Signature {
                signature_index: r.next16().or(Err("read failure"))?
            },
            "SourceFile" => SourceFile {
                sourcefile_index: r.next16().or(Err("read failure"))?
            },
            "SourceDebugExtension" => SourceDebugExtenson {
                debug_extension: {
                    let mut ans = Vec::with_capacity(r.next16().or(Err("read failure"))? as usize);
                    for _i in 0..attribute_length {
                        ans.push(r.next().or(Err("read failure"))?)
                    }
                    ans
                }
            },
            "LineNumberTable" => LineNumberTable {
                line_number_table: {
                    let len = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(LineNumberTableEntry {
                            start_pc: r.next16().or(Err("read failure"))?,
                            line_number: r.next16().or(Err("read failure"))?
                        })
                    }
                    ans
                }
            },
            "LocalVariableTable" => LocalVariableTable {
                local_variable_table: {
                    let len = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(LocalVariableTableEntry {
                            start_pc: r.next16().or(Err("read failure"))?,
                            length: r.next16().or(Err("read failure"))?,
                            name_index: r.next16().or(Err("read failure"))?,
                            descriptor_index: r.next16().or(Err("read failure"))?,
                            index: r.next16().or(Err("read failure"))?
                        })
                    }
                    ans
                }
            },
            "LocalVariableTypeTable" => LocalVariableTypeTable {
                local_variable_type_table: {
                    let len = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(LocalVariableTypeTableEntry {
                            start_pc: r.next16().or(Err("read failure"))?,
                            length: r.next16().or(Err("read failure"))?,
                            name_index: r.next16().or(Err("read failure"))?,
                            signature_index: r.next16().or(Err("read failure"))?,
                            index: r.next16().or(Err("read failure"))?
                        })
                    }
                    ans
                }
            },
            "Deprecated" => Deprecated,
            "RuntimeVisibleAnnotations" => RuntimeVisibleAnnotations {
                annotations: read_annotations(r)?
            },
            "RuntimeInvisibleAnnotations" => RuntimeInvisibleAnnotations {
                annotations: read_annotations(r)?
            },
            "RuntimeVisibleParameterAnnotations" => RuntimeVisibleParameterAnnotations {
                parameter_annotations: {
                    let num = r.next().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(read_annotations(r)?);
                    }
                    ans
                }
            },
            "RuntimeInvisibleParameterAnnotations" => RuntimeInvisibleParameterAnnotations {
                parameter_annotations: {
                    let num = r.next().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(read_annotations(r)?);
                    }
                    ans
                }
            },
            "RuntimeVisibleTypeAnnotations" => RuntimeVisibleTypeAnnotations {
                annotations: read_type_annotations(r)?
            },
            "RuntimeInvisibleTypeAnnotations" => RuntimeInvisibleTypeAnnotations {
                annotations: read_type_annotations(r)?
            },
            "AnnotationDefault" => AnnotationDefault {
                default_value: read_element_value(r)?
            },
            "BootstrapMethods" => BootstrapMethods {
                bootstrap_methods: {
                    let num = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(BootstrapMethodsEntry {
                            bootstrap_method_ref: r.next16().or(Err("read failure"))?,
                            bootstrap_arguments: {
                                let numb = r.next16().or(Err("read failure"))?;
                                let mut bas = Vec::with_capacity(numb as usize);
                                for _j in 0..numb {
                                    bas.push(r.next16().or(Err("read failure"))?);
                                }
                                bas
                            }
                        });
                    }
                    ans
                }
            },
            "MethodParameters" => MethodParameters {
                parameters: {
                    let num = r.next16().or(Err("read failure"))?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(MethodParameterEntry {
                            name_index: r.next16().or(Err("read failure"))?,
                            access_flags: r.next16().or(Err("read failure"))?
                        });
                    }
                    ans
                }
            },
            _ => return Err("invalid attribute name")
        });
    }
    Ok(ans)
}

fn read_type_annotations(r: &mut JavaClassReader) -> Result<Vec<TypeAnnotation>, &'static str> {
    let len = r.next16().or(Err("read failure"))?;
    let mut ans = Vec::with_capacity(len as usize);
    for _i in 0..len {
        ans.push(read_type_annotation(r)?);
    }
    Ok(ans)
}

fn read_type_annotation(r: &mut JavaClassReader) -> Result<TypeAnnotation, &'static str> {
    let target_type = r.next().or(Err("read failure"))?;
    let target_info = match target_type {
        0x00 | 0x01 => TargetInfo::TypeParameterTarget { type_parameter_index: r.next().or(Err("read failure"))? },
        0x10 => TargetInfo::SupertypeTarget { supertype_index: r.next16().or(Err("read failure"))? },
        0x11 | 0x12 => TargetInfo::TypeParameterBoundTarget { type_parameter_index: r.next().or(Err("read failure"))?, bound_index: r.next().or(Err("read failure"))? },
        0x13 | 0x14 | 0x15 => TargetInfo::EmptyTarget,
        0x16 => TargetInfo::FormalParameterTarget { formal_parameter_index: r.next().or(Err("read failure"))? },
        0x17 => TargetInfo::ThrowsTarget { throws_type_index: r.next16().or(Err("read failure"))? },
        0x40 | 0x41 => TargetInfo::LocalVarTarget {
            table: {
                let len = r.next16().or(Err("read failure"))?;
                let mut ans = Vec::with_capacity(r.next16().or(Err("read failure"))? as usize);
                for _i in 0..len {
                    ans.push(LocalVarTagetTableEntry {
                        start_pc: r.next16().or(Err("read failure"))?,
                        length: r.next16().or(Err("read failure"))?,
                        index: r.next16().or(Err("read failure"))?
                    });
                }
                ans
            }
        },
        0x42 => TargetInfo::CatchTarget { exception_table_index: r.next16().or(Err("read failure"))? },
        0x43 | 0x44 | 0x45 | 0x46 => TargetInfo::OffsetTarget { offset: r.next16().or(Err("read failure"))? },
        0x47 | 0x48 | 0x49 | 0x4A | 0x4B => TargetInfo::TypeArgumentTarget { offset: r.next16().or(Err("read failure"))?, type_argument_index: r.next().or(Err("read failure"))? },
        _ => return Err("Bad target info tag")
    };
    let type_path = TypePath {
        path: {
            let num = r.next().or(Err("read failure"))?;
            let mut ans = Vec::with_capacity(num as usize);
            for _i in 0..num {
                ans.push(TypePathEntry {
                    type_path_kind: r.next().or(Err("read failure"))?,
                    type_argument_index: r.next().or(Err("read failure"))?
                })
            }
            ans
        }
    };
    let type_index = r.next16().or(Err("read failure"))?;
    let num_ev = r.next16().or(Err("read failure"))?;
    let mut element_value_pairs = Vec::with_capacity(num_ev as usize);
    for _i in 0..num_ev {
        element_value_pairs.push(ElementValuePair {
            element_name_index: r.next16().or(Err("read failure"))?,
            value: read_element_value(r)?
        });
    }
    Ok(TypeAnnotation {
        target_info: target_info,
        target_path: type_path,
        type_index: type_index,
        element_value_pairs: element_value_pairs
    })
}

fn read_annotations(r: &mut JavaClassReader) -> Result<Vec<Annotation>, &'static str> {
    let len = r.next16().or(Err("read failure"))?;
    let mut ans = Vec::with_capacity(len as usize);
    for _i in 0..len {
        ans.push(read_annotation(r)?);
    }
    Ok(ans)
}

fn read_annotation(r: &mut JavaClassReader) -> Result<Annotation, &'static str> {
    let type_index = r.next16().or(Err("read failure"))?;
    let len = r.next16().or(Err("read failure"))?;
    let mut ans = Vec::with_capacity(len as usize);
    for _i in 0..len {
        ans.push(ElementValuePair {
            element_name_index: r.next16().or(Err("read failure"))?,
            value: read_element_value(r)?
        })
    }
    Ok(Annotation {
        type_index: type_index,
        element_value_pairs: ans
    })
}

fn read_element_value(r: &mut JavaClassReader) -> Result<ElementValue, &'static str> {
    let tag = r.next().or(Err("read failure"))?;
    Ok(match tag as char {
        'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z' | 's' => ElementValue::ConstValueIndex(r.next16().or(Err("read failure"))?),
        'e' => ElementValue::EnumConstValue {
            type_name_index: r.next16().or(Err("read failure"))?,
            const_name_index: r.next16().or(Err("read failure"))?
        },
        'c' => ElementValue::ClassInfoIndex(r.next16().or(Err("read failure"))?),
        '@' => ElementValue::AnnotationValue(read_annotation(r)?),
        '[' => ElementValue::ArrayValue({
            let num = r.next16().or(Err("read failure"))?;
            let mut ans = Vec::with_capacity(num as usize);
            for _i in 0..num {
                ans.push(read_element_value(r)?);
            }
            ans
        }),
        _ => return Err("invalid elementvalue tag")
    })
}

fn read_verification_type_info(r: &mut JavaClassReader) -> Result<VerificationTypeInfo, &'static str> {
    let tag = r.next().or(Err("read failure"))?;
    Ok(match tag {
        0 => VerificationTypeInfo::Top,
        1 => VerificationTypeInfo::Integer,
        2 => VerificationTypeInfo::Float,
        5 => VerificationTypeInfo::Null,
        6 => VerificationTypeInfo::UninitializedThis,
        7 => VerificationTypeInfo::Object { cpool_index: r.next16().or(Err("read failure"))? },
        8 => VerificationTypeInfo::UninitializedVariable { offset: r.next16().or(Err("read failure"))? },
        4 => VerificationTypeInfo::Long,
        3 => VerificationTypeInfo::Double,
        _ => return Err("bad verification type info number")
    })
}

fn malformed() -> io::Result<JavaClass> {
    Err(io::Error::new(io::ErrorKind::InvalidData, "Malformed class file"))
}

pub struct JavaClassReader {
    file: File,
    buffer: [u8; 1],
    dist: u32
}

impl JavaClassReader {
    fn new(file_name: &str) -> io::Result<JavaClassReader> {
        Ok(JavaClassReader {file: File::open(file_name)?, buffer: [0; 1], dist: 0})
    }
    pub fn next(&mut self) -> io::Result<u8> {
        self.file.read(&mut self.buffer)?;
        self.dist+=1;
        Ok(self.buffer[0])
    }
    pub fn next16(&mut self) -> io::Result<u16> {
        Ok(build_u16!(self.next()?, self.next()?))
    }
    pub fn next32(&mut self) -> io::Result<u32> {
        Ok(build_u32!(self.next()?, self.next()?, self.next()?, self.next()?))
    }
    pub fn next64(&mut self) -> io::Result<u64> {
        Ok(((self.next32()? as u64) << 32) | (self.next32()? as u64))
    }
    pub fn dist(&self) -> u32 {
        self.dist
    }
}