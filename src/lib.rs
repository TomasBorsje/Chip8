use wasm_bindgen::prelude::*;

const PIXEL_COLUMNS: usize = 64;
const PIXEL_ROWS: usize = 32;
const SCREEN_SIZE: usize = PIXEL_COLUMNS * PIXEL_ROWS; // 64 columns, 32 rows
const PROG_STARTING_ADDRESS: u16 = 0x200;
const FONT_SET: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80, 0xF0, 0xF0,
    0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0, 0x10, 0xF0, 0xF0, 0x80,
    0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90, 0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0,
    0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0, 0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80,
    0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0, 0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
];

enum OpCode {
    ClearScreen,                             // 00E0
    PopSubroutine,                           // 00EE
    Jump { nnn: u16 },                       // 1NNN
    CallSubroutine { nnn: u16 },             // 2NNN
    SkipIfEqual { x: u8, nn: u8 },           // 3XNN
    SkipIfNotEqual { x: u8, nn: u8 },        // 4XNN
    SkipIfRegisterEqual { x: u8, y: u8 },    // 5XY0
    SkipIfRegisterNotEqual { x: u8, y: u8 }, // 9XY0
    SetRegister { x: u8, nn: u8 },           // 6XNN
    AddValueToRegister { x: u8, nn: u8 },    // 7XNN
    Set { x: u8, y: u8 },                    // 8XY0
    BinaryOr { x: u8, y: u8 },               // 8XY1
    BinaryAnd { x: u8, y: u8 },              // 8XY2
    LogicalXor { x: u8, y: u8 },             // 8XY3
    Add { x: u8, y: u8 },                    // 8XY4
    SubtractXY { x: u8, y: u8 },             // 8XY5
    SubtractYX { x: u8, y: u8 },             // 8XY7
    ShiftRight { x: u8, y: u8 },             // 8XY6
    ShiftLeft { x: u8, y: u8 },              // 8XYE
    SetIndexRegister { nnn: u16 },           // ANNN
    JumpWithOffset { nnn: u16 },             // BNNN
    Random { x: u8, nn: u8 },                // CXNN
    Draw { x: u8, y: u8, n: u8 },            // DXYN
    SkipOneIfKey { x: u8 },                  // EX9E
    SkipOneIfNotKey { x: u8 },               // EXA1
    SetRegisterToDelayTimer { x: u8 },       // FX07
    SetDelayTimerToRegister { x: u8 },       // FX15
    SetSoundTimerToRegister { x: u8 },       // FX18
    AddRegisterToIndex { x: u8 },            // FX1E
    GetKey { x: u8 },                        // FX0A
    FontCharacter { x: u8 },                 // FX29
    BinaryCodedDecimalConversion { x: u8 },  // FX33
    StoreRegistersToMemory { x: u8 },        // FX55
    LoadRegistersFromMemory { x: u8 },       // FX65
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct Chip8Machine {
    memory: [u8; 4096],
    program_counter: u16,
    index_register: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    sound_timer: u8,
    registers: [u8; 16],
    current_input: Option<u8>,
    display: [bool; SCREEN_SIZE],
}

#[wasm_bindgen]
impl Chip8Machine {
    pub fn new(program: &[u8]) -> Option<Chip8Machine> {
        let mut chip_machine = Chip8Machine {
            memory: [0; 4096],
            program_counter: PROG_STARTING_ADDRESS,
            index_register: 0,
            stack: vec![],
            delay_timer: 0,
            sound_timer: 0,
            registers: [0; 16],
            current_input: None,
            display: [false; 64 * 32],
        };
        chip_machine.memory[0x050..0x0A0].copy_from_slice(&FONT_SET); // Copy font set into memory

        for (index, byte) in program.iter().enumerate() {
            chip_machine.memory[PROG_STARTING_ADDRESS as usize + index] = *byte;
        }
        println!("Loaded {0} bytes into memory", program.len());

        Some(chip_machine)
    }

    // Call this 60 times a second!
    pub fn decrement_timers(&mut self) {
        // Decrement both timers if they're above 0
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    pub fn input(&mut self, key: u8) {
        self.current_input = Some(key);
    }

    pub fn cycle(&mut self) {
        // Fetch instruction at the PC
        let (instruction_one, instruction_two) = self.fetch();

        // Decode the instruction
        let operation = self
            .decode(instruction_one, instruction_two)
            .expect("Unknown OpCode number");

        // Execute the instruction
        self.execute(operation);
        self.current_input = None;
    }

    fn fetch(&mut self) -> (u8, u8) {
        let instruction_one: u8 = self.memory[self.program_counter as usize];
        let instruction_two: u8 = self.memory[self.program_counter as usize + 1];
        self.program_counter += 2;

        (instruction_one, instruction_two)
    }

    fn decode(&mut self, instruction_one: u8, instruction_two: u8) -> Result<OpCode, String> {
        let instruction_code: u8 = (instruction_one >> 4) & 0b_00001111;
        let x: u8 = instruction_one & 0b_00001111; // Register 1 addr
        let y: u8 = (instruction_two >> 4) & 0b_00001111; // Register 2 addr
        let n: u8 = instruction_two & 0b_00001111;
        let nn: u8 = instruction_two;
        let nnn: u16 = ((x as u16) << 8) + nn as u16;

        match instruction_code {
            0x0 => match (x, n) {
                (0x0, 0x0) => Ok(OpCode::ClearScreen),
                (0x0, 0xE) => Ok(OpCode::PopSubroutine),
                (_, _) => panic!("Unrecognised 00E_ opcode with nibble {n}"),
            },
            0x1 => Ok(OpCode::Jump { nnn }),
            0x2 => Ok(OpCode::CallSubroutine { nnn }),
            0x3 => Ok(OpCode::SkipIfEqual { x, nn }),
            0x4 => Ok(OpCode::SkipIfNotEqual { x, nn }),
            0x5 => Ok(OpCode::SkipIfRegisterEqual { x, y }),
            0x6 => Ok(OpCode::SetRegister { x, nn }),
            0x7 => Ok(OpCode::AddValueToRegister { x, nn }),
            0x8 => match n {
                0x0 => Ok(OpCode::Set { x, y }),
                0x1 => Ok(OpCode::BinaryOr { x, y }),
                0x2 => Ok(OpCode::BinaryAnd { x, y }),
                0x3 => Ok(OpCode::LogicalXor { x, y }),
                0x4 => Ok(OpCode::Add { x, y }),
                0x5 => Ok(OpCode::SubtractXY { x, y }),
                0x6 => Ok(OpCode::ShiftLeft { x, y }),
                0x7 => Ok(OpCode::SubtractYX { x, y }),
                0xE => Ok(OpCode::ShiftRight { x, y }),
                _ => panic!("Unrecognised 8XYN opcode with nibble {n}"),
            },
            0x9 => Ok(OpCode::SkipIfRegisterNotEqual { x, y }),
            0xA => Ok(OpCode::SetIndexRegister { nnn }),
            0xC => Ok(OpCode::Random {x, nn}),
            0xD => Ok(OpCode::Draw { x, y, n }),
            0xE => match nn {
                0x9E => Ok(OpCode::SkipOneIfKey {x}),
                0xA1 => Ok(OpCode::SkipOneIfNotKey {x}),
                _ => panic!("Unrecognised EXNN opcode with nibble {nn}"),
            }
            0xF => match nn {
                0x07 => Ok(OpCode::SetRegisterToDelayTimer {x}),
                0x0A => Ok(OpCode::GetKey{x}),
                0x15 => Ok(OpCode::SetDelayTimerToRegister {x}),
                0x18 => Ok(OpCode::SetSoundTimerToRegister {x}),
                0x1E => Ok(OpCode::AddRegisterToIndex {x}),
                0x29 => Ok(OpCode::FontCharacter {x}),
                0x33 => Ok(OpCode::BinaryCodedDecimalConversion {x}),
                0x55 => Ok(OpCode::StoreRegistersToMemory {x}),
                0x65 => Ok(OpCode::LoadRegistersFromMemory {x}),
                _ => panic!("Unrecognised FXNN opcode with nibble {nn}"),
            }
            _ => panic!("Could not decode instruction with ID {instruction_code}"),
        }
    }

    fn execute(&mut self, operation: OpCode) {
        match operation {
            OpCode::ClearScreen => {
                self.display.fill(false);
            }
            OpCode::Jump { nnn } => {
                self.program_counter = nnn;
            }
            OpCode::SetRegister { x, nn } => {
                self.registers[x as usize] = nn;
            }
            OpCode::AddValueToRegister { x, nn } => {
                self.registers[x as usize] += nn;
            }
            OpCode::SetIndexRegister { nnn } => {
                self.index_register = nnn;
            }
            OpCode::Draw { x, y, n } => {
                let x_coord = self.registers[x as usize] % PIXEL_COLUMNS as u8;
                let y_coord = self.registers[y as usize] % PIXEL_ROWS as u8;
                self.registers[0xF] = 0;

                // Draw the sprite, setting 0xF to 1 if anything was turned off by this
                for row in 0..n {
                    let pixel_y = y_coord + row;
                    if pixel_y as usize >= PIXEL_ROWS {
                        break;
                    }

                    let sprite_row = self.memory[(self.index_register + row as u16) as usize];
                    for bit in 0..8 {
                        let mask: u8 = (2u8).pow(7 - bit);
                        let enable_pixel: bool = (sprite_row) | mask == sprite_row;

                        let pixel_x = x_coord + bit as u8;
                        if pixel_x as usize >= PIXEL_COLUMNS {
                            continue;
                        } // Stop if we're about to go off screen

                        let pixel_index = pixel_y as usize * PIXEL_ROWS + pixel_x as usize;

                        if enable_pixel {
                            if self.display[pixel_index] {
                                // If we're about to turn off a pixel, set 0xF to 1
                                self.registers[0xF] = 1;
                            }
                            self.display[pixel_index] = !self.display[pixel_index];
                        }
                    }
                }
            }
            OpCode::PopSubroutine => {}
            OpCode::CallSubroutine { .. } => {}
            OpCode::SkipIfEqual { .. } => {}
            OpCode::SkipIfNotEqual { .. } => {}
            OpCode::SkipIfRegisterEqual { .. } => {}
            OpCode::SkipIfRegisterNotEqual { .. } => {}
            OpCode::Set { .. } => {}
            OpCode::BinaryOr { .. } => {}
            OpCode::BinaryAnd { .. } => {}
            OpCode::LogicalXor { .. } => {}
            OpCode::Add { .. } => {}
            OpCode::SubtractXY { .. } => {}
            OpCode::SubtractYX { .. } => {}
            OpCode::ShiftRight { .. } => {}
            OpCode::ShiftLeft { .. } => {}
            OpCode::JumpWithOffset { .. } => {}
            OpCode::Random { .. } => {}
            OpCode::SkipOneIfKey { .. } => {}
            OpCode::SkipOneIfNotKey { .. } => {}
            OpCode::SetRegisterToDelayTimer { .. } => {}
            OpCode::SetDelayTimerToRegister { .. } => {}
            OpCode::SetSoundTimerToRegister { .. } => {}
            OpCode::AddRegisterToIndex { .. } => {}
            OpCode::GetKey { .. } => {}
            OpCode::FontCharacter { .. } => {}
            OpCode::BinaryCodedDecimalConversion { .. } => {}
            OpCode::StoreRegistersToMemory { .. } => {}
            OpCode::LoadRegistersFromMemory { .. } => {}
        }
    }

    pub fn get_display(&self) -> String {
        let mut output: String = String::new();

        for row in 0..PIXEL_ROWS {
            for col in 0..PIXEL_COLUMNS {
                output.push(match self.display[row * PIXEL_COLUMNS + col] {
                    true => '■',
                    false => '□',
                });
            }
            output.push('\n');
        }
        output
    }
}
