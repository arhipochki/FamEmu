mod cpu;

use crate::cpu::cpu::CPU;

fn main() {
    let other_cpu: CPU = CPU::new();

    println!("{:#?}", other_cpu);
}
