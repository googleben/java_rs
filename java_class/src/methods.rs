use attributes::Attribute;

/// struct representing the MethodInfo struct
/// for more information refer to the [JVM specification](https://docs.oracle.com/javase/specs/jvms/se8/html/index.html)
#[derive(Debug)]
pub struct MethodInfo {
    pub access_flags: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes: Vec<Attribute>
}

pub enum AccessFlags {
    Public       = 0x0001,
    Private      = 0x0002,
    Protected    = 0x0004,
    Static       = 0x0008,
    Final        = 0x0010,
    Synchronized = 0x0020,
    Bridge       = 0x0040,
    Varargs      = 0x0080,
    Native       = 0x0100,
    Abstract     = 0x0400,
    Strict       = 0x0800,
    Synthetic    = 0x1000
}