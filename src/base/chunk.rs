use super::{ByteCode, Instruction, ShrString, Value};

// ========================================================================== //
//  ChunkError
// ========================================================================== //

#[derive(Debug, thiserror::Error)]
pub enum ChunkError {
    #[error("ip {0} out of range {1}")]
    IpOutOfRange(usize, usize),

    #[error("constant index {0} out of range {1}")]
    ConstantOutOfRange(usize, usize),
    
    #[error("invalid bytecode {0}")]
    InvalidByteCode(u8),
    
    #[error("expected string constant")]
    ExpectedStringConstant,
}

// ========================================================================== //
//  Chunk
// ========================================================================== //

/// A chunk of bytecode — the unit of compilation.
///
/// Internally it stores a compact byte stream (`codes`) and a constant pool
/// (`constants`), but the public API works exclusively with high-level
/// [`Instruction`] values so that neither the compiler nor the VM need to
/// worry about encoding / decoding.
pub struct Chunk {
    pub codes: Vec<u8>,
    pub constants: Vec<Value>,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            codes: vec![],
            constants: vec![],
        }
    }
}

impl Default for Chunk {
    fn default() -> Self {
        Self::new()
    }
}

impl Chunk {
    /// Encode and append a single instruction to this chunk.
    pub fn write_instruction(&mut self, inst: Instruction) {
        match inst {
            // simple opcodes
            Instruction::Return => self.write_op(ByteCode::Return),
            Instruction::Pop => self.write_op(ByteCode::Pop),
            Instruction::Nil => self.write_op(ByteCode::Nil),
            Instruction::True => self.write_op(ByteCode::True),
            Instruction::False => self.write_op(ByteCode::False),
            Instruction::Negate => self.write_op(ByteCode::Negate),
            Instruction::Not => self.write_op(ByteCode::Not),
            Instruction::Add => self.write_op(ByteCode::Add),
            Instruction::Sub => self.write_op(ByteCode::Sub),
            Instruction::Mul => self.write_op(ByteCode::Mul),
            Instruction::Div => self.write_op(ByteCode::Div),
            Instruction::Equal => self.write_op(ByteCode::Equal),
            Instruction::NotEqual => self.write_op(ByteCode::NotEqual),
            Instruction::Greater => self.write_op(ByteCode::Greater),
            Instruction::GreaterEqual => self.write_op(ByteCode::GreaterEqual),
            Instruction::Less => self.write_op(ByteCode::Less),
            Instruction::LessEqual => self.write_op(ByteCode::LessEqual),

            // constant 
            Instruction::Constant(value) => {
                self.write_const_op(ByteCode::Constant, value);
            }

            // globals
            Instruction::DefineGlobal(name) => {
                self.write_const_op(ByteCode::DefineGlobal, Value::String(name));
            }
            Instruction::GetGlobal(name) => {
                self.write_const_op(ByteCode::GetGlobal, Value::String(name));
            }
            Instruction::SetGlobal(name) => {
                self.write_const_op(ByteCode::SetGlobal, Value::String(name));
            }

            // locals — 1-byte stack-slot index
            Instruction::GetLocal(index) => {
                assert!(index <= u16::MAX as usize, "Too many constants in one chunk!");
                self.write_op(ByteCode::GetLocal);
                self.write_u16(index as u16);
            }
            Instruction::SetLocal(index) => {
                assert!(index <= u16::MAX as usize, "Too many constants in one chunk!");
                self.write_op(ByteCode::SetLocal);
                self.write_u16(index as u16);
            }
        
            Instruction::JumpIfFalse(offset) => {
                assert!(offset <= u16::MAX as usize, "Too much code to jump over.");
                self.write_op(ByteCode::JumpIfFalse);
                self.write_u16(offset as u16);
            }
            Instruction::Jump(offset) => {
                assert!(offset <= u16::MAX as usize, "Too much code to jump over.");
                self.write_op(ByteCode::Jump);
                self.write_u16(offset as u16);
            }
            Instruction::Loop(offset) => {
                assert!(offset <= u16::MAX as usize, "Too much code to jump over.");
                self.write_op(ByteCode::Loop);
                self.write_u16(offset as u16);
            }
        
            Instruction::Call(arg_count) => {
                assert!(arg_count <= 256, "Too much args.");
                self.write_op(ByteCode::Call);
                self.write_byte(arg_count as u8);
            }

            Instruction::Closure { function, upvalues } => {
                self.write_const_op(ByteCode::Closure, function);
                self.write_byte(upvalues.len() as u8);
                for uv in upvalues {
                    self.write_byte(uv.is_local as u8);
                    self.write_byte(uv.index as u8);
                }
            }

            Instruction::GetUpvalue(slot) => {
                assert!(slot < u8::MAX as usize, "Too much upvalues.");
                self.write_op(ByteCode::GetUpvalue);
                self.write_byte(slot as u8);
            }
            Instruction::SetUpvalue(slot) => {
                assert!(slot < u8::MAX as usize, "Too much upvalues.");
                self.write_op(ByteCode::SetUpvalue);
                self.write_byte(slot as u8);
            }
            Instruction::CloseUpvalue => {
                self.write_op(ByteCode::CloseUpvalue);
            }

            Instruction::Class(class_name) => {
                self.write_const_op(ByteCode::Class, Value::String(class_name));
            }
            Instruction::SetProperty(class_name) => {
                self.write_const_op(ByteCode::SetProperty, Value::String(class_name));
            }
            Instruction::GetProperty(class_name) => {
                self.write_const_op(ByteCode::GetProperty, Value::String(class_name));
            }
            Instruction::Method(method_name) => {
                self.write_const_op(ByteCode::Method, Value::String(method_name));
            }
        }
    }

    /// Decode the instruction at the given bytecode pointer, advancing `ip`.
    pub fn read_instruction(&self, ip: &mut usize) -> Result<Instruction, ChunkError> {
        let byte = self.read_byte(ip)?;
        let opcode = ByteCode::try_from(byte)
            .map_err(|_| ChunkError::InvalidByteCode(byte))?;

        match opcode {
            ByteCode::Return => Ok(Instruction::Return),
            ByteCode::Pop => Ok(Instruction::Pop),
            ByteCode::Nil => Ok(Instruction::Nil),
            ByteCode::True => Ok(Instruction::True),
            ByteCode::False => Ok(Instruction::False),
            ByteCode::Negate => Ok(Instruction::Negate),
            ByteCode::Not => Ok(Instruction::Not),
            ByteCode::Add => Ok(Instruction::Add),
            ByteCode::Sub => Ok(Instruction::Sub),
            ByteCode::Mul => Ok(Instruction::Mul),
            ByteCode::Div => Ok(Instruction::Div),
            ByteCode::Equal => Ok(Instruction::Equal),
            ByteCode::NotEqual => Ok(Instruction::NotEqual),
            ByteCode::Greater => Ok(Instruction::Greater),
            ByteCode::GreaterEqual => Ok(Instruction::GreaterEqual),
            ByteCode::Less => Ok(Instruction::Less),
            ByteCode::LessEqual => Ok(Instruction::LessEqual),

            ByteCode::Constant => {
                let value = self.read_constant(ip)?;
                Ok(Instruction::Constant(value))
            }
            ByteCode::DefineGlobal => {
                let name = self.read_string_constant(ip)?;
                Ok(Instruction::DefineGlobal(name))
            }
            ByteCode::GetGlobal => {
                let name = self.read_string_constant(ip)?;
                Ok(Instruction::GetGlobal(name))
            }
            ByteCode::SetGlobal => {
                let name = self.read_string_constant(ip)?;
                Ok(Instruction::SetGlobal(name))
            }

            ByteCode::GetLocal => {
                let index = self.read_u16(ip)?;
                Ok(Instruction::GetLocal(index as usize))
            }
            ByteCode::SetLocal => {
                let index = self.read_u16(ip)?;
                Ok(Instruction::SetLocal(index as usize))
            }

            ByteCode::JumpIfFalse => {
                let index = self.read_u16(ip)?;
                Ok(Instruction::JumpIfFalse(index as usize))
            }
            ByteCode::Jump => {
                let index = self.read_u16(ip)?;
                Ok(Instruction::Jump(index as usize))
            }
            ByteCode::Loop => {
                let index = self.read_u16(ip)?;
                Ok(Instruction::Loop(index as usize))
            }
            
            ByteCode::Call => {
                let arg_count = self.read_byte(ip)?;
                Ok(Instruction::Call(arg_count as usize))
            }

            ByteCode::Closure => {
                let function = self.read_constant(ip)?;
                let upvalue_count = self.read_byte(ip)? as usize;
                let mut upvalues = Vec::with_capacity(upvalue_count);
                for _ in 0..upvalue_count {
                    let is_local = self.read_byte(ip)? != 0;
                    let index = self.read_byte(ip)? as usize;
                    upvalues.push(crate::UpvalueDesc { is_local, index });
                }
                Ok(Instruction::Closure { function, upvalues })
            }

            ByteCode::GetUpvalue => {
                let slot = self.read_byte(ip)? as usize;
                Ok(Instruction::GetUpvalue(slot))
            }
            ByteCode::SetUpvalue => {
                let slot = self.read_byte(ip)? as usize;
                Ok(Instruction::SetUpvalue(slot))
            }
            ByteCode::CloseUpvalue => {
                Ok(Instruction::CloseUpvalue)
            }

            ByteCode::Class => {
                let name = self.read_string_constant(ip)?;
                Ok(Instruction::Class(name))
            }
            ByteCode::GetProperty => {
                let field_name = self.read_string_constant(ip)?;
                Ok(Instruction::GetProperty(field_name))
            }
            ByteCode::SetProperty => {
                let field_name = self.read_string_constant(ip)?;
                Ok(Instruction::SetProperty(field_name))
            }
            ByteCode::Method => {
                let method_name = self.read_string_constant(ip)?;
                Ok(Instruction::Method(method_name))
            }
        }
    }

    fn write_op(&mut self, opcode: ByteCode) {
        self.codes.push(opcode as u8);
    }

    /// Write an opcode followed by a 2-byte LE constant-pool index.
    fn write_const_op(&mut self, opcode: ByteCode, value: Value) {
        let index = self.add_constant(value);
        assert!(index <= u16::MAX as usize, "Too many constants in one chunk!");
        self.write_op(opcode);
        self.write_u16(index as u16);
    }

    fn write_u16(&mut self, value: u16) {
        let bytes = value.to_le_bytes();
        self.codes.push(bytes[0]);
        self.codes.push(bytes[1]);
    }

    fn write_byte(&mut self, value: u8) {
        self.codes.push(value);
    }

    fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1
    }

    fn read_byte(&self, ip: &mut usize) -> Result<u8, ChunkError> {
        let byte = self
            .codes
            .get(*ip)
            .cloned()
            .ok_or(ChunkError::IpOutOfRange(*ip, self.codes.len()))?;
        *ip += 1;
        Ok(byte)
    }

    fn read_u16(&self, ip: &mut usize) -> Result<u16, ChunkError> {
        let b1 = self.read_byte(ip)?;
        let b2 = self.read_byte(ip)?;
        Ok(u16::from_le_bytes([b1, b2]))
    }

    fn read_constant(&self, ip: &mut usize) -> Result<Value, ChunkError> {
        let index = self.read_u16(ip)? as usize;
        self.constants
            .get(index)
            .cloned()
            .ok_or(ChunkError::ConstantOutOfRange(index, self.constants.len()))
    }

    fn read_string_constant(&self, ip: &mut usize) -> Result<ShrString, ChunkError> {
        match self.read_constant(ip)? {
            Value::String(s) => Ok(s),
            _ => Err(ChunkError::ExpectedStringConstant),
        }
    }
}
