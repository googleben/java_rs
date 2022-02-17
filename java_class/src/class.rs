#![allow(clippy::clippy::many_single_char_names)]
use attributes::*;
use attributes::Attribute::*;
use bytecode_tools::*;
use cp::ConstantPool;
use cp_info::*;
use cp_info::CPInfo::*;
use fields::FieldInfo;
use methods::MethodInfo;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::str;
use std::string::String;

use crate::cp::CPIndex;

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

/// An enum containing the access flags of a Java Class
pub enum AccessFlags {
    Public = 0x0001,
    Final = 0x0010,
    Super = 0x0020,
    Interface = 0x0200,
    Abstract = 0x0400,
    Synthetic = 0x1000,
    Annotation = 0x2000,
    Enum = 0x4000,
}

#[derive(Debug)]
pub struct JavaClass {
    pub minor_version: u16,
    pub major_version: u16,
    pub constant_pool: ConstantPool,
    pub access_flags: u16,
    pub this_class: CPIndex,
    pub super_class: CPIndex,
    pub interfaces: Vec<CPIndex>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub attributes: Vec<Attribute>,
}

impl JavaClass {
    pub fn empty() -> JavaClass {
        JavaClass {
            minor_version: 0,
            major_version: 0,
            constant_pool: ConstantPool::default(),
            access_flags: 0,
            this_class: CPIndex::default(),
            super_class: CPIndex::default(),
            interfaces: Vec::new(),
            fields: Vec::new(),
            methods: Vec::new(),
            attributes: Vec::new(),
        }
    }

    pub fn new(file_name: &str) -> io::Result<JavaClass> {
        JavaClass::build(JavaClassReader::new(file_name)?)
    }

    pub fn new_from_reader<T: Read>(reader: T) -> io::Result<JavaClass> {
        JavaClass::build(JavaClassReader::new_from_reader(reader)?)
    }

    pub fn new_from_bytes(bytes: Vec<u8>) -> io::Result<JavaClass> {
        JavaClass::build(JavaClassReader::new_from_bytes(bytes)?)
    }

    fn build(mut r: JavaClassReader) -> io::Result<JavaClass> {
        let magic = r.next32()?;
        if magic != 0xCAFEBABE { return malformed("Wrong magic number"); }
        let minor_version = r.next16()?;
        let major_version = r.next16()?;
        let cp_count = r.next16()?;
        let cp = build_cp(&mut r, cp_count)?;

        let access_flags = r.next16()?;
        let this_class = r.next16()?.into();
        let super_class = r.next16()?.into();
        let interfaces_count = r.next16()?;
        let mut interfaces = vec!();
        for _ in 0..interfaces_count {
            interfaces.push(r.next16()?.into());
        }
        let fields_count = r.next16()?;
        let mut fields = Vec::with_capacity(fields_count as usize);
        for _i in 0..fields_count {
            fields.push(FieldInfo {
                access_flags: r.next16()?,
                name_index: r.next16()?.into(),
                descriptor_index: r.next16()?.into(),
                attributes: {
                    let x = read_attributes(&mut r, &cp);
                    match x {
                        Ok(a) => a,
                        Err(s) => return Err(io::Error::new(io::ErrorKind::InvalidData, s))
                    }
                },
            })
        }
        let methods_count = r.next16()?;
        let mut methods = Vec::with_capacity(methods_count as usize);
        for _i in 0..methods_count {
            methods.push(MethodInfo {
                access_flags: r.next16()?,
                name_index: r.next16()?.into(),
                descriptor_index: r.next16()?.into(),
                attributes: {
                    let x = read_attributes(&mut r, &cp);
                    match x {
                        Ok(a) => a,
                        Err(s) => return Err(io::Error::new(io::ErrorKind::InvalidData, s))
                    }
                },
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
            attributes,
        })
    }

    pub fn is_interface(&self) -> bool {
        self.access_flags & AccessFlags::Interface as u16 != 0
    }

    pub fn get_name(&self) -> String {
        match self.constant_pool[self.this_class] {
            Class { name_index } => {
                match &self.constant_pool[name_index] {
                    Utf8 { bytes, .. } => {
                        str::from_utf8(&bytes).unwrap().to_owned()
                    }
                    _ => "Class Pool index did not point to Utf8".to_owned()
                }
            }
            _ => "Class Pool index did not point to Utf8".to_owned()
        }
    }
}

/// Reads in the constant pool of a class
fn build_cp(r: &mut JavaClassReader, cp_count: u16) -> io::Result<ConstantPool> {
    let mut cp_vec: Vec<CPInfo> = vec!();
    //we have to use an iterator so we can skip indices for Double and Long
    let mut iter = 0..cp_count - 1;
    while let Some(i) = iter.next() {
        let tag = r.next8()?;
        if !(1..=18).contains(&tag) || tag == 2 || tag == 13 || tag == 14 || tag == 17 {
            return malformed("Invalid constant pool tag");
        }
        let x: CPInfo = match tag {
            7 => Class { name_index: r.next16()?.into() },
            9 => Fieldref { class_index: r.next16()?.into(), name_and_type_index: r.next16()?.into() },
            10 => Methodref { class_index: r.next16()?.into(), name_and_type_index: r.next16()?.into() },
            11 => InterfaceMethodref { class_index: r.next16()?.into(), name_and_type_index: r.next16()?.into() },
            8 => CPInfo::String { string_index: r.next16()?.into() },
            3 => Integer { bytes: r.next32()? },
            4 => Float { bytes: r.next32()? },
            5 => Long { bytes: r.next64()? },
            6 => Double { bytes: r.next64()? },
            12 => NameAndType { name_index: r.next16()?.into(), descriptor_index: r.next16()?.into() },
            1 => {
                let length = r.next16()?;
                let mut bytes: Vec<u8> = vec!();
                for j in 0..length {
                    bytes.insert(j as usize, r.next8()?);
                }
                Utf8 { length, bytes }
            }
            15 => MethodHandle { reference_kind: r.next8()?, reference_index: r.next16()?.into() },
            16 => MethodType { descriptor_index: r.next16()?.into() },
            18 => InvokeDynamic { bootstrap_method_attr_index: r.next16()?.into(), name_and_type_index: r.next16()?.into() },
            _ => panic!("Unreachable code, wildcard case reached in exhaustive match") //unreachable
        };
        //deal with the awful fact that long and double constant pool entries are actually 2 entries
        //seriously, what were they thinking
        //can't we have changed that by now?? class files don't have to be backwards-compatible
        //throwing a huge wrench in my machine here
        match x {
            Double { .. } | Long { .. } => {
                cp_vec.insert(i as usize, x);
                iter.next();
                cp_vec.insert((i + 1) as usize, LongDoubleDummy);
            }
            _ => {
                cp_vec.insert(i as usize, x);
            }
        };
    };
    Ok(ConstantPool::new_with_info(cp_vec))
}

/// reads a String based on a list of bytes representing a Java-style modified UTF-8 list of bytes
/// JVM specification §4.4.7
pub fn read_string(bytes: &[u8]) -> String {
    let mut chars = Vec::with_capacity(bytes.len());
    let mut index: usize = 0;
    while index < bytes.len() {
        let b = bytes[index];
        if b == 0b11101101 {
            //supplemental character, requires fixing for rust
            index += 1;
            let v = bytes[index];
            index += 1;
            let w = bytes[index];
            index += 2;
            let y = bytes[index];
            index += 1;
            let z = bytes[index];
            let code_point = ((v as u32 & 0x0f) << 16) + ((w as u32 & 0x3f) << 10) + ((y as u32 & 0x0f) << 6) + (z & 0x3f) as u32;
            chars.push((0b11110000 + ((code_point >> 15) as u8 & 0b00000111)) as char);
            chars.push((0b10000000 + ((code_point >> 12) as u8 & 0b00111111)) as char);
            chars.push((0b10000000 + ((code_point >> 6) as u8 & 0b00111111)) as char);
            chars.push((0b10000000 + (code_point as u8 & 0b00111111)) as char);
        } else {
            chars.push(b as char);
        }
        index += 1;
    }
    chars.into_iter().collect()
}

/// reads in the attributes of an arbitrary class file element
fn read_attributes(r: &mut JavaClassReader, cp: &ConstantPool) -> io::Result<Vec<Attribute>> {
    let num = r.next16()?;
    let mut ans = Vec::with_capacity(num as usize);
    for _i in 0..num {
        let name_index = r.next16()?.into();
        let attribute_length = r.next32()?;
        let name = match &cp[name_index] {
            Utf8 { length: _, bytes } => read_string(bytes),
            _ => return malformed("Attribute tag was not Utf8")
        };
        ans.push(match name.as_str() {
            "ConstantValue" => ConstantValue { constantvalue_index: r.next16()?.into() },
            "Code" => Code {
                max_stack: r.next16()?,
                max_locals: r.next16()?,
                code: {
                    let code_len = r.next32()?;
                    let mut ans = Vec::with_capacity(code_len as usize);
                    let start = r.dist();
                    while r.dist() - start < code_len {
                        ans.push(to_opcode(r, start).ok_or_else(|| malformed_inner("Invalid bytecode"))?);
                    }
                    ans
                },
                exception_table: {
                    let len = r.next16()?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(ExceptionTableEntry {
                            start_pc: r.next16()?,
                            end_pc: r.next16()?,
                            handler_pc: r.next16()?,
                            catch_type: r.next16()?.into(),
                        });
                    }
                    ans
                },
                attributes: {
                    read_attributes(r, &cp)?
                },
            },
            "StackMapTable" => StackMapTable {
                entries: {
                    let num = r.next16()?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        let tag = r.next8()?;
                        ans.push(match tag {
                            0..=63 => StackMapFrame::SameFrame { offset_delta: tag },
                            64..=127 => StackMapFrame::SameLocals1Item {
                                offset_delta: tag - 64,
                                stack: read_verification_type_info(r)?,
                            },
                            247 => StackMapFrame::SameLocals1ItemExtended {
                                offset_delta: r.next16()?,
                                stack: read_verification_type_info(r)?,
                            },
                            248..=250 => StackMapFrame::ChopFrame {
                                absent_locals: 251 - tag,
                                offset_delta: r.next16()?,
                            },
                            251 => StackMapFrame::SameFrameExtended {
                                offset_delta: r.next16()?
                            },
                            252..=254 => StackMapFrame::AppendFrame {
                                offset_delta: r.next16()?,
                                locals: {
                                    let mut ans = vec!();
                                    for _i in 0..(tag - 251) {
                                        ans.push(read_verification_type_info(r)?);
                                    }
                                    ans
                                },
                            },
                            255 => StackMapFrame::FullFrame {
                                offset_delta: r.next16()?,
                                locals: {
                                    let mut ans = vec!();
                                    for _i in 0..r.next16()? {
                                        ans.push(read_verification_type_info(r)?);
                                    }
                                    ans
                                },
                                stack: {
                                    let mut ans = vec!();
                                    for _i in 0..r.next16()? {
                                        ans.push(read_verification_type_info(r)?);
                                    }
                                    ans
                                },
                            },
                            _ => return malformed("Invalid stackmapframe tag")
                        });
                    }
                    ans
                }
            },
            "Exceptions" => Exceptions {
                exception_index_table: {
                    let num = r.next16()?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(r.next16()?);
                    }
                    ans
                }
            },
            "InnerClasses" => InnerClasses {
                classes: {
                    let num = r.next16()?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(InnerClassInfo {
                            inner_class_info_index: r.next16()?.into(),
                            outer_class_info_index: r.next16()?.into(),
                            inner_name_index: r.next16()?.into(),
                            inner_class_access_flags: r.next16()?,
                        })
                    }
                    ans
                }
            },
            "EnclosingMethod" => EnclosingMethod {
                class_index: r.next16()?.into(),
                method_index: r.next16()?.into(),
            },
            "Synthetic" => Synthetic,
            "Signature" => Signature {
                signature_index: r.next16()?.into()
            },
            "SourceFile" => SourceFile {
                sourcefile_index: r.next16()?.into()
            },
            "SourceDebugExtension" => SourceDebugExtenson {
                debug_extension: {
                    let mut ans = Vec::with_capacity(r.next16()? as usize);
                    for _i in 0..attribute_length {
                        ans.push(r.next8()?)
                    }
                    ans
                }
            },
            "LineNumberTable" => LineNumberTable {
                line_number_table: {
                    let len = r.next16()?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(LineNumberTableEntry {
                            start_pc: r.next16()?,
                            line_number: r.next16()?,
                        })
                    }
                    ans
                }
            },
            "LocalVariableTable" => LocalVariableTable {
                local_variable_table: {
                    let len = r.next16()?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(LocalVariableTableEntry {
                            start_pc: r.next16()?,
                            length: r.next16()?,
                            name_index: r.next16()?.into(),
                            descriptor_index: r.next16()?.into(),
                            index: r.next16()?,
                        })
                    }
                    ans
                }
            },
            "LocalVariableTypeTable" => LocalVariableTypeTable {
                local_variable_type_table: {
                    let len = r.next16()?;
                    let mut ans = Vec::with_capacity(len as usize);
                    for _i in 0..len {
                        ans.push(LocalVariableTypeTableEntry {
                            start_pc: r.next16()?,
                            length: r.next16()?,
                            name_index: r.next16()?.into(),
                            signature_index: r.next16()?.into(),
                            index: r.next16()?,
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
                    let num = r.next8()?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(read_annotations(r)?);
                    }
                    ans
                }
            },
            "RuntimeInvisibleParameterAnnotations" => RuntimeInvisibleParameterAnnotations {
                parameter_annotations: {
                    let num = r.next8()?;
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
                    let num = r.next16()?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(BootstrapMethodsEntry {
                            bootstrap_method_ref: r.next16()?.into(),
                            bootstrap_arguments: {
                                let numb = r.next16()?;
                                let mut bas = Vec::with_capacity(numb as usize);
                                for _j in 0..numb {
                                    bas.push(r.next16()?.into());
                                }
                                bas
                            },
                        });
                    }
                    ans
                }
            },
            "MethodParameters" => MethodParameters {
                parameters: {
                    let num = r.next16()?;
                    let mut ans = Vec::with_capacity(num as usize);
                    for _i in 0..num {
                        ans.push(MethodParameterEntry {
                            name_index: r.next16()?.into(),
                            access_flags: r.next16()?,
                        });
                    }
                    ans
                }
            },
            _ => return malformed("invalid attribute name")
        });
    }
    Ok(ans)
}

fn read_type_annotations(r: &mut JavaClassReader) -> io::Result<Vec<TypeAnnotation>> {
    let len = r.next16()?;
    let mut ans = Vec::with_capacity(len as usize);
    for _i in 0..len {
        ans.push(read_type_annotation(r)?);
    }
    Ok(ans)
}

fn read_type_annotation(r: &mut JavaClassReader) -> io::Result<TypeAnnotation> {
    let target_type = r.next8()?;
    let target_info = match target_type {
        0x00 | 0x01 => TargetInfo::TypeParameterTarget { type_parameter_index: r.next8()? },
        0x10 => TargetInfo::SupertypeTarget { supertype_index: r.next16()? },
        0x11 | 0x12 => TargetInfo::TypeParameterBoundTarget { type_parameter_index: r.next8()?, bound_index: r.next8()? },
        0x13 | 0x14 | 0x15 => TargetInfo::EmptyTarget,
        0x16 => TargetInfo::FormalParameterTarget { formal_parameter_index: r.next8()? },
        0x17 => TargetInfo::ThrowsTarget { throws_type_index: r.next16()? },
        0x40 | 0x41 => TargetInfo::LocalVarTarget {
            table: {
                let len = r.next16()?;
                let mut ans = Vec::with_capacity(r.next16()? as usize);
                for _i in 0..len {
                    ans.push(LocalVarTagetTableEntry {
                        start_pc: r.next16()?,
                        length: r.next16()?,
                        index: r.next16()?,
                    });
                }
                ans
            }
        },
        0x42 => TargetInfo::CatchTarget { exception_table_index: r.next16()? },
        0x43 | 0x44 | 0x45 | 0x46 => TargetInfo::OffsetTarget { offset: r.next16()? },
        0x47 | 0x48 | 0x49 | 0x4A | 0x4B => TargetInfo::TypeArgumentTarget { offset: r.next16()?, type_argument_index: r.next8()? },
        _ => return malformed("Bad target info tag")
    };
    let target_path = TypePath {
        path: {
            let num = r.next8()?;
            let mut ans = Vec::with_capacity(num as usize);
            for _i in 0..num {
                ans.push(TypePathEntry {
                    type_path_kind: r.next8()?,
                    type_argument_index: r.next8()?,
                })
            }
            ans
        }
    };
    let type_index = r.next16()?.into();
    let num_ev = r.next16()?;
    let mut element_value_pairs = Vec::with_capacity(num_ev as usize);
    for _i in 0..num_ev {
        element_value_pairs.push(ElementValuePair {
            element_name_index: r.next16()?.into(),
            value: read_element_value(r)?,
        });
    }
    Ok(TypeAnnotation {
        target_info,
        target_path,
        type_index,
        element_value_pairs,
    })
}

fn read_annotations(r: &mut JavaClassReader) -> io::Result<Vec<Annotation>> {
    let len = r.next16()?;
    let mut ans = Vec::with_capacity(len as usize);
    for _i in 0..len {
        ans.push(read_annotation(r)?);
    }
    Ok(ans)
}

fn read_annotation(r: &mut JavaClassReader) -> io::Result<Annotation> {
    let type_index = r.next16()?.into();
    let len = r.next16()?;
    let mut ans = Vec::with_capacity(len as usize);
    for _i in 0..len {
        ans.push(ElementValuePair {
            element_name_index: r.next16()?.into(),
            value: read_element_value(r)?,
        })
    }
    Ok(Annotation {
        type_index,
        element_value_pairs: ans,
    })
}

fn read_element_value(r: &mut JavaClassReader) -> io::Result<ElementValue> {
    let tag = r.next8()?;
    Ok(match tag as char {
        'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z' | 's' => ElementValue::ConstValueIndex(r.next16()?.into()),
        'e' => ElementValue::EnumConstValue {
            type_name_index: r.next16()?.into(),
            const_name_index: r.next16()?.into(),
        },
        'c' => ElementValue::ClassInfoIndex(r.next16()?.into()),
        '@' => ElementValue::AnnotationValue(read_annotation(r)?),
        '[' => ElementValue::ArrayValue({
            let num = r.next16()?;
            let mut ans = Vec::with_capacity(num as usize);
            for _i in 0..num {
                ans.push(read_element_value(r)?);
            }
            ans
        }),
        _ => return malformed("invalid elementvalue tag")
    })
}

fn read_verification_type_info(r: &mut JavaClassReader) -> io::Result<VerificationTypeInfo> {
    let tag = r.next8()?;
    Ok(match tag {
        0 => VerificationTypeInfo::Top,
        1 => VerificationTypeInfo::Integer,
        2 => VerificationTypeInfo::Float,
        5 => VerificationTypeInfo::Null,
        6 => VerificationTypeInfo::UninitializedThis,
        7 => VerificationTypeInfo::Object { cpool_index: r.next16()?.into() },
        8 => VerificationTypeInfo::UninitializedVariable { offset: r.next16()? },
        4 => VerificationTypeInfo::Long,
        3 => VerificationTypeInfo::Double,
        _ => return malformed("Bad verification type info number")
    })
}

fn malformed<T>(err: &str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::InvalidData, format!("Malformed class file: {}", err)))
}
fn malformed_inner(err: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("Malformed class file: {}", err))
}

/// an abstraction for reading bytes of a .class
pub struct JavaClassReader {
    buffer: Vec<u8>,
    dist: u32,
}

impl JavaClassReader {
    /// creates a JavaClassReader by using a file
    /// # Parameters:
    /// * file_name: the path of the file to read in
    fn new(file_name: &str) -> io::Result<JavaClassReader> {
        let mut buffer = Vec::with_capacity(::std::fs::metadata(file_name)?.len() as usize);
        File::open(file_name)?.read_to_end(&mut buffer)?;
        Ok(JavaClassReader { buffer, dist: 0 })
    }
    /// creates a JavaClassReader by using a Rust struct implementing Read
    fn new_from_reader<T: Read>(mut reader: T) -> io::Result<JavaClassReader> {
        let mut buffer = vec!();
        reader.read_to_end(&mut buffer)?;
        Ok(JavaClassReader { buffer, dist: 0 })
    }
    /// creates a JavaClassReader by using a Vec of bytes
    fn new_from_bytes(bytes: Vec<u8>) -> io::Result<JavaClassReader> {
        Ok(JavaClassReader { buffer: bytes, dist: 0 })
    }
    pub fn next8(&mut self) -> io::Result<u8> {
        if self.dist as usize >= self.buffer.len() {
            return malformed("Reached eof early");
        }
        let ans = self.buffer[self.dist as usize];
        self.dist += 1;
        Ok(ans)
    }
    pub fn next16(&mut self) -> io::Result<u16> {
        Ok(build_u16!(self.next8()?, self.next8()?))
    }
    pub fn next32(&mut self) -> io::Result<u32> {
        Ok(build_u32!(self.next8()?, self.next8()?, self.next8()?, self.next8()?))
    }
    pub fn next64(&mut self) -> io::Result<u64> {
        Ok(((self.next32()? as u64) << 32) | (self.next32()? as u64))
    }
    pub fn dist(&self) -> u32 {
        self.dist
    }
}