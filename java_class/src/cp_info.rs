/// enum containing all JVM cp_info structs
/// for more information refer to the [JVM specification](https://docs.oracle.com/javase/specs/jvms/se8/html/index.html)
#[derive(Debug)]
pub enum CPInfo {
    Class { name_index: u16 }, //name_index
    Fieldref { class_index: u16, name_and_type_index: u16 }, //class_index, name_and_type_index
    Methodref { class_index: u16, name_and_type_index: u16}, //class_index, name_and_type_index
    InterfaceMethodref { class_index: u16, name_and_type_index: u16 }, //class_index, name_and_type_index
    String { string_index: u16 }, //string_index
    Integer { bytes: u32 }, //bytes
    Float { bytes: u32 }, //bytes
    Long { bytes: u64 }, //bytes
    Double { bytes: u64 }, //bytes
    LongDoubleDummy,
    NameAndType { name_index: u16, descriptor_index: u16 }, //name_index, descriptor_index
    Utf8 { length: u16, bytes: Vec<u8> }, //length, bytes
    MethodHandle { reference_kind: u8, reference_index: u16 }, //reference_kind, reference_index
    MethodType { descriptor_index: u16 }, //descriptor_index
    InvokeDynamic { bootstrap_method_attr_index: u16, name_and_type_index: u16 } //bootstrap_method_attr_index, name_and_type_index
}

impl CPInfo {
    /// returns the byte tag of the given `CPInfo` variant
    pub fn tag(&self) -> u8 {
        match *self {
            CPInfo::Class { .. } => 7,
            CPInfo::Fieldref { .. } => 9,
            CPInfo::Methodref { .. } => 10,
            CPInfo::InterfaceMethodref { .. } => 11,
            CPInfo::String { .. } => 8,
            CPInfo::Integer { .. } => 3,
            CPInfo::Float { .. } => 4,
            CPInfo::Long { .. } => 5,
            CPInfo::LongDoubleDummy => 255,
            CPInfo::Double { .. } => 6,
            CPInfo::NameAndType { .. } => 12,
            CPInfo::Utf8 { .. } => 1,
            CPInfo::MethodHandle { .. } => 15,
            CPInfo::MethodType { .. } => 16,
            CPInfo::InvokeDynamic { .. } => 18
        }
    }
}