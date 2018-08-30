use std::ops::Index;
use cp_info::CPInfo;
use std::string;

#[derive(Debug)]
pub struct ConstantPool {
    cp: Vec<CPInfo>
}

impl ConstantPool {

    pub fn new(cpv: Vec<CPInfo>) -> Result<ConstantPool, Vec<string::String>> {
        let cp = ConstantPool {cp: cpv};
        Ok(cp)
    }

    pub fn len(&self) -> u16 {
        self.cp.len() as u16+1
    }

    pub fn items(&self) -> &Vec<CPInfo> {
        &self.cp
    }
    
}

impl Index<u16> for ConstantPool {

    type Output = CPInfo;

    fn index(&self, index: u16) -> &CPInfo {
        &self.cp[index as usize-1]
    }

}

