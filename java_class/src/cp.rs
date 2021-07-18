use cp_info::CPInfo;
use std::ops::Index;

/// A struct representing the constant pool of a class file.
/// 1-indexed (to emulate the Java constant pool)
#[derive(Debug)]
pub struct ConstantPool {
    cp: Vec<CPInfo>
}

impl Default for ConstantPool {
    /// Creates a new empty `ConstantPool`
    fn default() -> ConstantPool {
        ConstantPool { cp: Vec::new() }
    }
}

impl ConstantPool {
    /// Creates a new `ConstantPool` using existing `CPInfo`
    /// 
    /// # Arguments
    /// 
    /// * `cpv` - holds the `CPInfo` to initialize the constant pool with
    pub fn new_with_info(cpv: Vec<CPInfo>) -> ConstantPool {
        ConstantPool { cp: cpv }
    }

    /// Returns the 1-indexed length of the constant pool
    pub fn len(&self) -> u16 {
        self.cp.len() as u16 + 1
    }

    pub fn is_empty(&self) -> bool {
        self.cp.is_empty()
    }

    /// Returns a reference to the `Vec` containing the constants
    /// Use this for easy iteration
    pub fn items(&self) -> &Vec<CPInfo> {
        &self.cp
    }
}

impl Index<u16> for ConstantPool {
    type Output = CPInfo;

    fn index(&self, index: u16) -> &CPInfo {
        &self.cp[index as usize - 1]
    }
}

