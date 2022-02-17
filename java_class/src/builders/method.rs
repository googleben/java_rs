use crate::opcodes::Opcode;

pub struct MethodBuilder {
    code: Vec<Opcode>,
    params: Vec<u8>
}