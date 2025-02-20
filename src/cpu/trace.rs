use crate::cpu::cpu::{AddressingMode, CPU};
use crate::cpu::opcodes;
use crate::cpu::mem::Mem;

use std::collections::HashMap;
use std::format;

pub fn trace(cpu: &mut CPU) -> String {
    let ref opcodes: HashMap<u8, &'static opcodes::OpCode> = *opcodes::OPCODES_MAP;
    
    let start = cpu.program_counter;

    let code = cpu.mem_read(start);
    let opcode = opcodes.get(&code).expect(&format!("OpCode {:x} wasn't recognized!", code));
    
    let mut hex_dump = vec![code];

    let (mem_addr, stored_value) = match opcode.mode {
        AddressingMode::Immediate | AddressingMode::NoneAddressing => (0, 0),

        _ => {
            let addr = cpu.get_absolute_address(&opcode.mode, start + 1);

            (addr, cpu.mem_read(addr))
        }
    };

    let asm_opcode_with_address = match opcode.len {

        1 => match opcode.code {
            0x0A | 0x4A | 0x2A | 0x6A => format!("A "),
            _ => "".to_string()
        },
        2 => {
            let addr = cpu.mem_read(start + 1);
            hex_dump.push(addr);

            match opcode.mode {
                AddressingMode::Immediate => format!("#${:02X}", addr),
                
                AddressingMode::ZeroPage => format!("${:02X} = {:02X}", mem_addr, stored_value),
                
                AddressingMode::ZeroPage_X => format!(
                        "${:02X},X @{:02X} = {:02X}",
                        addr, mem_addr, stored_value
                ),
                AddressingMode::ZeroPage_Y => format!(
                        "${:02X},Y @{:02X} = {:02X}",
                        addr, mem_addr, stored_value
                ),
                AddressingMode::Indirect_X => format!(
                        "(${:02X},X) @{:02X} = {:04X} = {:02X}",
                        addr, (addr.wrapping_add(cpu.register_x)), mem_addr, stored_value
                ),
                AddressingMode::Indirect_Y => format!(
                        "(${:02X}),Y = {:04X} @{:04X} = {:02X}",
                        addr, (addr.wrapping_add(cpu.register_y)), mem_addr, stored_value
                ),
                AddressingMode::NoneAddressing => {
                    // Operations like JMP, BNE, BNQ, etc

                    let addr = (start as usize + 2).wrapping_add((addr as i8) as usize);

                    format!("${:04X}", addr)
                }

                _ => panic!(
                    "Unexpected addressing mode {:?} has ops-len 2. Code {:02X}",
                    opcode.mode, opcode.code
                )
            }
        },
        3 => {
            let low = cpu.mem_read(start + 1);
            let high = cpu.mem_read(start + 2);
        
            hex_dump.push(low);
            hex_dump.push(high);
        
            let addr = cpu.mem_read_u16(start + 1);

            match opcode.mode {
                AddressingMode::NoneAddressing => {
                    // JMP indirect
                    if opcode.code == 0x6C {
                        let jmp_addr = if addr & 0x00FF == 0x00FF {
                            let low = cpu.mem_read(addr);
                            let high = cpu.mem_read(addr & 0x00FF);
                            (high as u16) << 8 | (low as u16)
                        }
                        else {
                            cpu.mem_read_u16(addr)
                        };

                        format!("(${:04X}) = {:04X}", addr, jmp_addr)
                    }
                    else {
                        format!("${:04X}", addr)
                    }
                }
                AddressingMode::Absolute => format!(
                    "${:04X} = {:02X}", mem_addr, stored_value
                ),
                AddressingMode::Absolute_X => format!(
                    "${:04X},X @ {:04X} = {:02X}",
                    addr, mem_addr, stored_value
                ),
                AddressingMode::Absolute_Y => format!(
                    "${:04X},Y @ {:04X} = {:02X}",
                    addr, mem_addr, stored_value
                ),
                _ => panic!(
                    "Unexpected addressing mode {:?} has ops-len 3. Code {:02X}",
                    opcode.mode, opcode.code
                )
            }
        }
        _ => "".to_string()
    };

    let hex_string = hex_dump
                .iter()
                .map(|z| format!("{:02X}", z))
                .collect::<Vec<String>>()
                .join(" ");
    
    let asm_string = format!(
        "{:04X}  {:8} {: >4} {}", start, hex_string, opcode.mnemonic, asm_opcode_with_address
    ).trim().to_string();

    format!(
        "{:47} A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
        asm_string, cpu.register_a, cpu.register_x, cpu.register_y, cpu.status, cpu.stack_pointer
    )
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::bus::Bus;
    use crate::rom::Rom;
    use std::fs;
    #[test]
    fn test_format_trace() {
        let game_code = fs::read("/Users/alexey/Documents/Prog/Rust/FamEmu/nestest.nes").unwrap();
        let rom = Rom::new(&game_code).unwrap();
        let mut bus = Bus::new(rom);
        bus.mem_write(100, 0xa2);
        bus.mem_write(101, 0x01);
        bus.mem_write(102, 0xca);
        bus.mem_write(103, 0x88);
        bus.mem_write(104, 0x00);

        let mut cpu = CPU::new(bus);
        cpu.program_counter = 0x64;
        cpu.register_a = 1;
        cpu.register_x = 2;
        cpu.register_y = 3;
        let mut result: Vec<String> = vec![];
        cpu.run_with_callback(|cpu| {
            result.push(trace(cpu));
        });
        assert_eq!(
            "0064  A2 01     LDX #$01                        A:01 X:02 Y:03 P:24 SP:FD",
            result[0]
        );
        assert_eq!(
            "0066  CA        DEX                             A:01 X:01 Y:03 P:24 SP:FD",
            result[1]
        );
        assert_eq!(
            "0067  88        DEY                             A:01 X:00 Y:03 P:26 SP:FD",
            result[2]
        );
    }

    #[test]
    fn test_format_mem_access() {
        let game_code = fs::read("/Users/alexey/Documents/Prog/Rust/FamEmu/nestest.nes").unwrap();
        let rom = Rom::new(&game_code).unwrap();
        let mut bus = Bus::new(rom);
        // ORA ($33), Y
        bus.mem_write(100, 0x11);
        bus.mem_write(101, 0x33);

        //data
        bus.mem_write(0x33, 00);
        bus.mem_write(0x34, 04);

        //target cell
        bus.mem_write(0x400, 0xAA);

        let mut cpu = CPU::new(bus);
        cpu.program_counter = 0x64;
        cpu.register_y = 0;
        let mut result: Vec<String> = vec![];
        cpu.run_with_callback(|cpu| {
            result.push(trace(cpu));
        });
        assert_eq!(
            "0064  11 33     ORA ($33),Y = 0400 @ 0400 = AA  A:00 X:00 Y:00 P:24 SP:FD",
            result[0]
        );
    }
}