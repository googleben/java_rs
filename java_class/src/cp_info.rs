macro_rules! push_u16 {
    ($num:expr, $vec:expr) => {
        let tmp = $num;
        ($vec).push((tmp >> 8) as u8);
        ($vec).push(*tmp as u8);
    }
}

macro_rules! push_u32 {
    ($num:expr, $vec:expr) => {
        let tmp = $num;
        ($vec).push((tmp >> 24) as u8);
        ($vec).push((tmp >> 16) as u8);
        ($vec).push((tmp >> 8) as u8);
        ($vec).push(*tmp as u8);
    };
}

macro_rules! push_u64 {
    ($num:expr, $vec:expr) => {
        let tmp = $num;
        ($vec).push((tmp >> 56) as u8);
        ($vec).push((tmp >> 48) as u8);
        ($vec).push((tmp >> 40) as u8);
        ($vec).push((tmp >> 32) as u8);
        ($vec).push((tmp >> 24) as u8);
        ($vec).push((tmp >> 16) as u8);
        ($vec).push((tmp >> 8) as u8);
        ($vec).push(*tmp as u8);
    };
}

/// enum containing all JVM cp_info structs
/// for more information refer to the [JVM specification](https://docs.oracle.com/javase/specs/jvms/se8/html/index.html)
#[derive(Debug)]
pub enum CPInfo {
    Class { name_index: u16 },
    //name_index
    Fieldref { class_index: u16, name_and_type_index: u16 },
    //class_index, name_and_type_index
    Methodref { class_index: u16, name_and_type_index: u16 },
    //class_index, name_and_type_index
    InterfaceMethodref { class_index: u16, name_and_type_index: u16 },
    //class_index, name_and_type_index
    String { string_index: u16 },
    //string_index
    Integer { bytes: u32 },
    //bytes
    Float { bytes: u32 },
    //bytes
    Long { bytes: u64 },
    //bytes
    Double { bytes: u64 },
    //bytes
    LongDoubleDummy,
    NameAndType { name_index: u16, descriptor_index: u16 },
    //name_index, descriptor_index
    Utf8 { length: u16, bytes: Vec<u8> },
    //length, bytes
    MethodHandle { reference_kind: u8, reference_index: u16 },
    //reference_kind, reference_index
    MethodType { descriptor_index: u16 },
    //descriptor_index
    InvokeDynamic { bootstrap_method_attr_index: u16, name_and_type_index: u16 }, //bootstrap_method_attr_index, name_and_type_index
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

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut ans = vec!();
        if let CPInfo::LongDoubleDummy = self {
            return ans;
        }
        ans.push(self.tag());
        match self {
            CPInfo::Class { name_index } => {
                push_u16!(name_index, ans);
            },
            CPInfo::Fieldref { class_index, name_and_type_index } => {
                push_u16!(class_index, ans);
                push_u16!(name_and_type_index, ans);
            },
            CPInfo::Methodref { class_index, name_and_type_index } => {
                push_u16!(class_index, ans);
                push_u16!(name_and_type_index, ans);
            },
            CPInfo::InterfaceMethodref { class_index, name_and_type_index } => {
                push_u16!(class_index, ans);
                push_u16!(name_and_type_index, ans);
            },
            CPInfo::String { string_index } => {
                push_u16!(string_index, ans);
            },
            CPInfo::Integer { bytes } => {
                push_u32!(bytes, ans);
            },
            CPInfo::Float { bytes } => {
                push_u32!(bytes, ans);
            },
            CPInfo::Long { bytes } => {
                push_u64!(bytes, ans);
            },
            CPInfo::Double { bytes } => {
                push_u64!(bytes, ans);
            },
            CPInfo::NameAndType { name_index, descriptor_index } => {
                push_u16!(name_index, ans);
                push_u16!(descriptor_index, ans);
            },
            CPInfo::Utf8 { length, bytes } => {
                push_u16!(length, ans);
                ans.copy_from_slice(bytes);
            },
            CPInfo::MethodHandle { reference_kind, reference_index } => {
                ans.push(*reference_kind);
                push_u16!(reference_index, ans);
            },
            CPInfo::MethodType { descriptor_index } => {
                push_u16!(descriptor_index, ans);
            },
            CPInfo::InvokeDynamic { bootstrap_method_attr_index, name_and_type_index } => {
                push_u16!(bootstrap_method_attr_index, ans);
                push_u16!(name_and_type_index, ans);
            },
            CPInfo::LongDoubleDummy => {}
        }
        ans
    }
}