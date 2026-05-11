use std::{thread, time::{self}};

use chip8::Chip8Machine;

// Note: This code is not run as the crate compiles to a DLL at the moment.
fn main() {
    const HERTZ: u64 = 700;
    const FPS: u64 = 15;
    const FILEPATH: &str = "pages/bc_test.ch8";

    let program = std::fs::read(FILEPATH).expect("Couldn't read FILEPATH");
    let mut machine = Chip8Machine::new(&program[..], false).expect("Couldn't make Chip8Machine");
    let mut counter = 0;
    loop {
        machine.cycle();

        // Display every X cycles
        if counter == 0 {
            let display = machine.get_display();
            print!("{}[2J", 27 as char); // Clear console
            print!("{display}");
            counter = HERTZ/FPS;
        }
        else {
            counter -= 1;
        }

        // Sleep
        thread::sleep(time::Duration::from_micros(1_000_000/HERTZ));
    }
}
