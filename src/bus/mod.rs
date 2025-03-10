use crate::cpu::mem::Mem;
use crate::rom::Rom;

pub struct Bus {
    cpu_vram: [u8; 2048],
    rom: Rom,
}

impl Bus {
    pub fn new(rom: Rom) -> Self {
        Bus {
            cpu_vram: [0; 2048],
            rom: rom,
        }
    }

    pub fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= 0x8000;
        if self.rom.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            addr %= 0x4000;
        }

        self.rom.prg_rom[addr as usize]
    }
}

const RAM: u16 = 0x000;
const RAM_MIRROR_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_END: u16 = 0x3FFF;

impl Mem for Bus {
    fn mem_read(&self, addr: u16) -> u8 {
        match addr {
            RAM..=RAM_MIRROR_END => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize]
            }

            PPU_REGISTERS..=PPU_REGISTERS_END => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                //todo!("PPU isn't implemented yet")
                0
            }

            0x8000..=0xFFFF => self.read_prg_rom(addr),

            _ => {
                println!("Ignoring mem address at {}", addr);
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            RAM..=RAM_MIRROR_END => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize] = data
            }

            PPU_REGISTERS..=PPU_REGISTERS_END => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                //todo!("PPU isn't implemented yet")
            }

            0x8000..=0xFFFF => {
                panic!("Attemp to write to Cartrdige ROM space");
            }

            _ => {
                println!("Ignoring mem write-address at {}", addr);
            }
        }
    }
}
