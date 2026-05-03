use std::{thread, time::{self}};

use chip8::Chip8Machine;

fn main() {
    const HERTZ: u64 = 700;
    const FILEPATH: &str = "ibm.ch8";

    let mut machine = Chip8Machine::new(FILEPATH).unwrap_or_else(|err| panic!("Couldn't create Chip8Machine: {err}"));
    let mut counter = 0;

    loop {
        machine.cycle();

        // Display every X cycles
        if counter == 0 {
            let display = machine.get_display();
            print!("{}[2J", 27 as char); // Clear console
            print!("{display}");
            counter = HERTZ * 2;
        }

        // Sleep
        thread::sleep(time::Duration::from_micros(1_000_000/HERTZ));
    }
}
