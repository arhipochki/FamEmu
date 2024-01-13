use crate::cpu::opcodes;
use std::collections::HashMap;

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing
}

bitflags::bitflags! {
    /* # Status Register (P) http://wiki.nesdev.com/w/index.php/Status_flags
    
      7 6 5 4 3 2 1 0
      N V _ B D I Z C
      | | | | | | | +--- Carry Flag
      | | | | | | +----- Zero Flag
      | | | | | +------- Interrupt Disable
      | | | | +--------- Decimal Mode (not used on NES)
      | | | +----------- Break Command
      | | +------------- Break2 Command/Flag
      | +--------------- Overflow Flag
      +----------------- Negative Flag
    */

    #[derive(Debug, Clone)]
    pub struct CpuFlags: u8 {
        const CARRY                 = 0b00000001;
        const ZERO                  = 0b00000010;
        const INTERRUPT_DISABLE     = 0b00000100;
        const DECIMAL_MODE          = 0b00001000;
        const BREAK                 = 0b00010000;
        const BREAK_2               = 0b00100000;
        const OVERFLOW              = 0b01000000;
        const NEGATIVE              = 0b10000000;
    }
}

const STACK: u16 = 0x0100;
const STACK_RESET: u8 = 0xFD;

#[derive(Debug)]
pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: CpuFlags,
    pub program_counter: u16,
    pub stack_pointer: u8,
    memory: [u8; 0xFFFF]
}

pub trait Mem {
    fn mem_read(&self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&self, pos: u16) -> u16 {
        let low = self.mem_read(pos) as u16;
        let high = self.mem_read(pos + 1) as u16;

        (high << 8) | low
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let high = (data >> 8) as u8;
        let low = (data & 0xFF) as u8;

        self.mem_write(pos, low);
        self.mem_write(pos + 1, high);
    }
}

impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            status: CpuFlags::from_bits_truncate(0b100100),
            program_counter: 0,
            stack_pointer: STACK_RESET,
            memory: [0; 0xFFFF]
        }
    }
    
    fn get_operand_address(&self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => self.program_counter,

            AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,

            AddressingMode::Absolute => self.mem_read_u16(self.program_counter),

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_x) as u16;
                
                addr
            }

            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_y) as u16;
                
                addr
            }

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                
                addr
            }

            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_y as u16);
                
                addr
            }

            AddressingMode::Indirect_X => {
                let base = self.mem_read(self.program_counter);
                
                let ptr = base.wrapping_add(self.register_x);

                let low = self.mem_read(ptr as u16);
                let high = self.mem_read(ptr.wrapping_add(1) as u16);
                
                (high as u16) << 8 | (low as u16)
            }

            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.program_counter);

                let low = self.mem_read(base as u16);
                let high = self.mem_read(base.wrapping_add(1) as u16);

                let deref_base = (high as u16) << 8 | (low as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                
                deref
            }

            AddressingMode::NoneAddressing => {
                panic!("AddressingMode {:?} isn't supported!", mode);
            }
        }
    }

    // Call insert when statement is true or remove when statement is false
    fn set_status(&mut self, flag: CpuFlags, statement: bool) {
        self.status.set(flag, statement);
    }

    fn set_register_a(&mut self, value: u8) {
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn stack_pop(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.mem_read(STACK + self.stack_pointer as u16)
    }

    fn stack_push(&mut self, value: u8) {
        self.mem_write(STACK + self.stack_pointer as u16, value);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    fn stack_pop_u16(&mut self) -> u16 {
        let low = self.stack_pop() as u16;
        let high = self.stack_pop() as u16;

        high << 8 | low
    }

    fn stack_push_u16(&mut self, value: u16) {
        let high = (value >> 8) as u8;
        let low = (value & 0xFF) as u8;

        self.stack_push(high);
        self.stack_push(low);
    }
    
    // NOTE: we're ignoring decimal mode, because Ricoh CPU doesn't support it
    // http://www.righto.com/2012/12/the-6502-overflow-flag-explained.html
    fn add_to_register_a(&mut self, value: u8) {
        let sum = self.register_a as u16 + value as u16
                + (if self.status.contains(CpuFlags::CARRY) {
                    1
                } else {
                    0
                });
        
        let carry = sum > 0xFF;

        self.set_status(CpuFlags::CARRY, carry);

        let result = sum as u8;

        self.set_status(
            CpuFlags::OVERFLOW, 
            (value ^ result) & (result ^ self.register_a) & 0x80 != 0
        );

        self.set_register_a(result);
    }

    fn adc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        
        self.add_to_register_a(value);
    }

    fn sbc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr) as i8;

        self.set_register_a(value.wrapping_neg().wrapping_sub(1) as u8);
    }

    fn and(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.set_register_a(value & self.register_a);
    }

    fn asl(&mut self, mode: &AddressingMode) -> u8 {
        let addr = self.get_operand_address(mode);
        let mut data = self.mem_read(addr);

        self.set_status(CpuFlags::CARRY, data >> 7 == 1);

        data <<= 1;
        self.mem_write(addr, data);
        self.update_zero_and_negative_flags(data);

        data
    }

    fn asl_accumulator(&mut self) {
        let mut data = self.register_a;

        self.set_status(CpuFlags::CARRY, data >> 7 == 1);

        data <<= 1;

        self.set_register_a(data);
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        let and = value & self.register_a;
        
        self.set_status(CpuFlags::ZERO, and == 0);
        self.set_status(CpuFlags::NEGATIVE, value & CpuFlags::NEGATIVE.bits() > 0);
        self.set_status(CpuFlags::OVERFLOW, value & CpuFlags::OVERFLOW.bits() > 0);
    }

    fn branch(&mut self, condition: bool) {
        if condition {
            let jump = self.mem_read(self.program_counter) as i8;
            let jump_addr = self.program_counter
                            .wrapping_add(1)
                            .wrapping_add(jump as u16);
        
            self.program_counter = jump_addr;
        }
    }

    fn compare(&mut self, mode: &AddressingMode, compare_with: u8) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.set_status(CpuFlags::CARRY, value <= compare_with);

        self.update_zero_and_negative_flags(compare_with.wrapping_sub(value));
    }

    fn dec(&mut self, mode: &AddressingMode) -> u8 {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        value = value.wrapping_sub(1);
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);
        value
    }

    fn dex(&mut self) {
        self.register_x = self.register_x.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn dey(&mut self) {
        self.register_y = self.register_y.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn eor(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.set_register_a(value ^ self.register_a);
    }

    fn inc(&mut self, mode: &AddressingMode) -> u8 {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);

        value = value.wrapping_add(1);
        self.mem_write(addr, value);
        self.update_zero_and_negative_flags(value);

        value
    }

    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn iny(&mut self) {
        self.register_y = self.register_y.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn jmp_absolute(&mut self) {
        let addr = self.mem_read_u16(self.program_counter);
        self.program_counter = addr; 
    }

    fn jmp_indirect(&mut self) {
        // 6502 bug mode with with page boundary:
        //  if address $3000 contains $40, $30FF contains $80, and $3100 contains $50,
        // the result of JMP ($30FF) will be a transfer of control
        // to $4080 rather than $5080 as you intended
        // i.e. the 6502 took the low byte of the address from $30FF and the high byte from $3000

        let addr = self.mem_read_u16(self.program_counter);
        let indirect_ref = if addr & 0x00FF == 0x00FF {
            let low = self.mem_read(addr);
            let high = self.mem_read(addr & 0xFF00);
            
            (high as u16) << 8 | (low as u16)
        } else {
            self.mem_read_u16(addr)
        };

        self.program_counter = indirect_ref;
    }

    fn jsr(&mut self) {
        self.stack_push_u16(self.program_counter + 1);
        let target_addr = self.mem_read_u16(self.program_counter);
        self.program_counter = target_addr;
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.set_register_a(value);
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn lsr(&mut self, mode: &AddressingMode) -> u8 {
        let addr = self.get_operand_address(mode);
        let mut data = self.mem_read(addr);

        self.set_status(CpuFlags::CARRY, data & 1 == 1);

        data >>= 1;
        self.mem_write(addr, data);
        self.update_zero_and_negative_flags(data);

        data
    }

    fn lsr_accumulator(&mut self) {
        let mut data = self.register_a;

        self.set_status(CpuFlags::CARRY, data & 1 == 1);

        data >>= 1;

        self.set_register_a(data);
    }

    fn pha(&mut self) {
        self.stack_push(self.register_a);
    }

    fn pla(&mut self) {
        let value = self.stack_pop();
        self.set_register_a(value);
    }

    fn php(&mut self) {
        //http://wiki.nesdev.com/w/index.php/CPU_status_flag_behavior
        let mut flags = self.status.clone();

        flags.insert(CpuFlags::BREAK);
        flags.insert(CpuFlags::BREAK_2);

        self.stack_push(flags.bits());
    }

    fn plp(&mut self) {
        self.status = CpuFlags::from_bits_truncate(self.stack_pop());
        self.status.remove(CpuFlags::BREAK);
        self.status.insert(CpuFlags::BREAK_2);
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn tay(&mut self) {
        self.register_y = self.register_a;
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn txs(&mut self) {
        self.stack_pointer = self.register_x;
    }

    fn tsx(&mut self) {
        self.register_x = self.stack_pointer;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn txa(&mut self) {
        self.register_a = self.register_x;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn tya(&mut self) {
        self.register_a = self.register_y;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn ora(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.set_register_a(value | self.register_a);
    }

    fn rts(&mut self) {
        self.program_counter = self.stack_pop_u16() + 1;
    }

    fn rti(&mut self) {
        self.status = CpuFlags::from_bits_truncate(self.stack_pop());
        self.status.remove(CpuFlags::BREAK);
        self.status.insert(CpuFlags::BREAK_2);

        self.program_counter = self.stack_pop_u16();
    }

    fn rol(&mut self, mode: &AddressingMode) -> u8 {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        let old_carry = self.status.contains(CpuFlags::CARRY);

        self.set_status(CpuFlags::CARRY, value >> 7 == 1);

        value <<= 1;

        if old_carry {
            value |= 1;
        }

        self.mem_write(addr, value);
        // update NEGATIVE flag
        self.set_status(CpuFlags::NEGATIVE, value >> 7 == 1);

        value
    }

    fn rol_accumulator(&mut self) {
        let mut value = self.register_a;
        let old_carry = self.status.contains(CpuFlags::CARRY);

        self.set_status(CpuFlags::CARRY, value >> 7 == 1);

        value <<= 1;

        if old_carry {
            value |= 1;
        }

        self.set_register_a(value);
    }

    fn ror(&mut self, mode: &AddressingMode) -> u8 {
        let addr = self.get_operand_address(mode);
        let mut value = self.mem_read(addr);
        let old_carry = self.status.contains(CpuFlags::CARRY);

        self.set_status(CpuFlags::CARRY, value & 1 == 1);

        value >>= 1;

        if old_carry {
            value |= CpuFlags::NEGATIVE.bits();
        }

        self.mem_write(addr, value);
        
        // update NEGATIVE flag
        self.set_status(CpuFlags::NEGATIVE, value >> 7 == 1);

        value
    }

    fn ror_accumulator(&mut self) {
        let mut value = self.register_a;
        let old_carry = self.status.contains(CpuFlags::CARRY);

        self.set_status(CpuFlags::CARRY, value & 1 == 1);

        value >>= 1;

        if old_carry {
            value |= CpuFlags::NEGATIVE.bits();
        }

        self.set_register_a(value);
    }

    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    fn stx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_x);
    }

    fn sty(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_y);
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        self.set_status(CpuFlags::ZERO, result == 0);

        self.set_status(CpuFlags::NEGATIVE, result >> 7 == 1);
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.stack_pointer = STACK_RESET;
        self.status = CpuFlags::from_bits_truncate(0b100100);

        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x0600 .. (0x0600 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xFFFC, 0x0600);
    }

    pub fn run(&mut self) {
        self.run_with_callback(|_| {});
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where 
        F: FnMut(&mut CPU)
    {
        let ref opcodes: HashMap<u8, &'static opcodes::OpCode> = *opcodes::OPCODES_MAP;

        loop {
            let code = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let current_program_counter_state = self.program_counter;

            let opcode = opcodes.get(&code).expect(&format!("OpCode {:x} wasn't recognized!", code));
            //let opcode = opcodes.get(&code).unwrap();

            println!("code {:x}", &code);

            match code {
                // BRK
                0x00 => return,
                
                // NOP
                0xEA => {},

                0x8A => self.txa(),

                0xAA => self.tax(),
                
                0xE8 => self.inx(),

                0xCA => self.dex(),
                
                0xA8 => self.tay(),

                0x98 => self.tya(),

                0x88 => self.dey(),

                0xC8 => self.iny(),

                0x69 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => {
                    self.adc(&opcode.mode);
                }

                0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => {
                    self.and(&opcode.mode);
                }
                
                0x0A => self.asl_accumulator(),
                
                0x06 | 0x16 | 0x0E | 0x1E => {
                    self.asl(&opcode.mode);
                }

                0x24 | 0x2C => {
                    self.bit(&opcode.mode);
                }

                // BPL
                0x10 => self.branch(!self.status.contains(CpuFlags::NEGATIVE)),
                
                // BMI
                0x30 => self.branch(self.status.contains(CpuFlags::NEGATIVE)),
                
                // BVC
                0x50 => self.branch(!self.status.contains(CpuFlags::OVERFLOW)),
                
                // BVS
                0x70 => self.branch(self.status.contains(CpuFlags::OVERFLOW)),
                
                // BCC
                0x90 => self.branch(!self.status.contains(CpuFlags::CARRY)),
                
                // BCS
                0xB0 => self.branch(self.status.contains(CpuFlags::CARRY)),
                
                // BNE
                0xD0 => self.branch(!self.status.contains(CpuFlags::ZERO)),

                // BEQ
                0xF0 => self.branch(self.status.contains(CpuFlags::ZERO)),

                // CMP
                0xC9 | 0xC5 | 0xD5 | 0xCD | 0xDD | 0xD9 | 0xC1 | 0xD1 => {
                    self.compare(&opcode.mode, self.register_a);
                }

                // CPX
                0xE0 | 0xE4 | 0xEC => {
                    self.compare(&opcode.mode, self.register_x);
                }

                // CPY
                0xC0 | 0xC4 | 0xCC => {
                    self.compare(&opcode.mode, self.register_y);
                }

                0xC6 | 0xD6 | 0xCE | 0xDE => {
                    self.dec(&opcode.mode);
                }

                0x49 | 0x45 | 0x55 | 0x4D | 0x5D | 0x59 | 0x41 | 0x51 => {
                    self.eor(&opcode.mode);
                }

                // CLC
                0x18 => self.set_status(CpuFlags::CARRY, false),

                // SEC
                0x38 => self.set_status(CpuFlags::CARRY, true),

                // CLI
                0x58 => self.set_status(CpuFlags::INTERRUPT_DISABLE, false),

                // SEI
                0x78 => self.set_status(CpuFlags::INTERRUPT_DISABLE, true),

                // CLV
                0xB8 => self.set_status(CpuFlags::OVERFLOW, false),

                // CLD
                0xD8 => self.set_status(CpuFlags::DECIMAL_MODE, false),

                // SED
                0xF8 => self.set_status(CpuFlags::DECIMAL_MODE, true),

                0xE6 | 0xF6 | 0xEE | 0xFE => {
                    self.inc(&opcode.mode);
                }

                0x4C => self.jmp_absolute(),

                0x6C => self.jmp_indirect(),

                0x20 => self.jsr(),

                0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
                    self.lda(&opcode.mode);
                }
                
                0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => {
                    self.ldx(&opcode.mode);
                }

                0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => {
                    self.ldy(&opcode.mode);
                }

                0x4A => self.lsr_accumulator(),
                
                0x46 | 0x56 | 0x4E | 0x5E => {
                    self.lsr(&opcode.mode);
                }
                
                0x09 | 0x05 | 0x15 | 0x0D | 0x1D | 0x19 | 0x01 | 0x11 => {
                    self.ora(&opcode.mode);
                }

                0x2A => self.rol_accumulator(),
                
                0x26 | 0x36 | 0x2E | 0x3E => {
                    self.rol(&opcode.mode);
                }

                0x6A => self.ror_accumulator(),

                0x66 | 0x76 | 0x6E | 0x7E => {
                    self.ror(&opcode.mode);
                }

                0x40 => self.rti(),

                0x60 => self.rts(),

                0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD | 0xF9 | 0xE1 | 0xF1 => {
                    self.sbc(&opcode.mode);
                }

                0x9A => self.txs(),
                
                0xBA => self.tsx(),
                
                0x48 => self.pha(),
                
                0x68 => self.pla(),
                
                0x08 => self.php(),

                0x28 => self.plp(),

                0x85 | 0x95 | 0x8D | 0x9D | 0x99 | 0x81 | 0x91 => {
                    self.sta(&opcode.mode);
                }

                0x86 | 0x96 | 0x8E => {
                    self.stx(&opcode.mode);
                }
                
                0x84 | 0x94 | 0x8C => {
                    self.sty(&opcode.mode);
                }

                _ => todo!()
            }

            if current_program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }

            callback(self);
        }
    }
}