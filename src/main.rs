mod cpu;
mod bus;
mod rom;

use std::fs;

use bus::Bus;
use cpu::trace::trace;
use cpu::cpu::CPU;
use cpu::mem::Mem;
use rand::Rng;
use rom::Rom;
use sdl2::event::Event;
use sdl2::EventPump;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::TextureAccess;

fn handle_user_input(cpu: &mut CPU, event_pump: &mut EventPump) {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                println!("Quit");
                std::process::exit(0)
            },
            Event::KeyDown { keycode: Some(Keycode::W), .. } => {
                cpu.mem_write(0xFF, 0x77);
            },
            Event::KeyDown { keycode: Some(Keycode::A), .. } => {
                cpu.mem_write(0xFF, 0x61);
            },
            Event::KeyDown { keycode: Some(Keycode::S), .. } => {
                cpu.mem_write(0xFF, 0x73);
            },
            Event::KeyDown { keycode: Some(Keycode::D), .. } => {
                cpu.mem_write(0xFF, 0x64);
            }

            _ => { /* DO NOTHING */}
        }
    }
}

fn color(byte: u8) -> Color {
    match byte {
        0 => Color::BLACK,
        1 => Color::WHITE,
        2 | 9 => Color::GRAY,
        3 | 10 => Color::RED,
        4 | 11 => Color::GREEN,
        5 | 12 => Color::BLUE,
        6 | 13 => Color::MAGENTA,
        7 | 14 => Color::YELLOW,
        _ => Color::CYAN
    }
}

fn read_screen_state(cpu: &CPU, frame: &mut [u8; 32 * 32 * 3]) -> bool {
    let mut frame_idx = 0;
    let mut update = false;

    for i in 0x0200..0x0600 {
        let color_idx = cpu.mem_read(i as u16);
        let (b1, b2, b3) = color(color_idx).rgb();

        if frame[frame_idx] != b1 || frame[frame_idx + 1] != b2 || frame[frame_idx + 2] != b3 {
            frame[frame_idx] = b1;
            frame[frame_idx + 1] = b2;
            frame[frame_idx + 2] = b3;

            update = true;
        }
        frame_idx += 3;
    }

    update
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
            .window("FamEmu: Testing Snake", 320 as u32, 320 as u32)
            .position_centered()
            .build().unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(10.0, 10.0).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture(PixelFormatEnum::RGB24, TextureAccess::Static, 32, 32).unwrap();

    let game_code = fs::read("/Users/alexey/Documents/Prog/Rust/FamEmu/snake.nes").unwrap();
    let rom = Rom::new(&game_code).unwrap();
    let bus = Bus::new(rom);

    let mut cpu = CPU::new(bus);
    cpu.reset();

    let mut screen_state = [0 as u8; 32 * 32 * 3];
    let mut rng = rand::thread_rng();

    cpu.run_with_callback(move |cpu| {
        println!("{}", trace(cpu));

        handle_user_input(cpu, &mut event_pump);
        cpu.mem_write(0xFE, rng.gen_range(1..16));
        
        if read_screen_state(cpu, &mut screen_state) {
            texture.update(None, &screen_state, 32 * 3).unwrap();
            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
        }

        std::thread::sleep(std::time::Duration::new(0, 100_000));
    });
}