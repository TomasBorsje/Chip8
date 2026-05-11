use rand::prelude::*;
use wasm_bindgen::prelude::*;

const PIXEL_COLUMNS: usize = 64;
const PIXEL_ROWS: usize = 32;
const SCREEN_SIZE: usize = PIXEL_COLUMNS * PIXEL_ROWS; // 64 columns, 32 rows
const PROG_STARTING_ADDRESS: u16 = 0x200;
const FONT_SET_STARTING_ADDRESS: u16 = 0x050;
const FONT_SET: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80, 0xF0, 0xF0,
    0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0, 0x10, 0xF0, 0xF0, 0x80,
    0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90, 0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0,
    0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0, 0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80,
    0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0, 0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
];
const CARRY_FLAG_REG: usize = 0xF;

enum OpCode {
    ClearScreen,                                // 00E0
    PopSubroutine,                              // 00EE
    Jump { nnn: u16 },                          // 1NNN
    CallSubroutine { nnn: u16 },                // 2NNN
    SkipIfEqual { x: u8, nn: u8 },              // 3XNN
    SkipIfNotEqual { x: u8, nn: u8 },           // 4XNN
    SkipIfRegisterEqual { x: u8, y: u8 },       // 5XY0
    SkipIfRegisterNotEqual { x: u8, y: u8 },    // 9XY0
    SetRegister { x: u8, nn: u8 },              // 6XNN
    AddValueToRegister { x: u8, nn: u8 },       // 7XNN
    SetRegisterToRegister { x: u8, y: u8 },     // 8XY0
    BinaryOr { x: u8, y: u8 },                  // 8XY1
    BinaryAnd { x: u8, y: u8 },                 // 8XY2
    LogicalXor { x: u8, y: u8 },                // 8XY3
    AddRegisterToRegister { x: u8, y: u8 },     // 8XY4
    SubtractXY { x: u8, y: u8 },                // 8XY5
    SubtractYX { x: u8, y: u8 },                // 8XY7
    ShiftRight { x: u8, y: u8 },                // 8XY6
    ShiftLeft { x: u8, y: u8 },                 // 8XYE
    SetIndexRegister { nnn: u16 },              // ANNN
    JumpWithOffset { x: u8, nnn: u16 }, // BNNN
    Random { x: u8, nn: u8 },                   // CXNN
    Draw { x: u8, y: u8, n: u8 },               // DXYN
    SkipIfKey { x: u8 },                     // EX9E
    SkipIfNotKey { x: u8 },                  // EXA1
    SetRegisterToDelayTimer { x: u8 },          // FX07
    SetDelayTimerToRegister { x: u8 },          // FX15
    SetSoundTimerToRegister { x: u8 },          // FX18
    AddRegisterToIndex { x: u8 },               // FX1E
    GetKey { x: u8 },                           // FX0A
    FontCharacter { x: u8 },                    // FX29
    BinaryCodedDecimalConversion { x: u8 },     // FX33
    StoreRegistersToMemory { x: u8 },           // FX55
    LoadRegistersFromMemory { x: u8 },          // FX65
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
    original_behaviour: bool, // Use modern shift opcode behaviour
    rand: ThreadRng,
}

#[wasm_bindgen]
impl Chip8Machine {
    pub fn new(program: &[u8], original_behaviour: bool) -> Option<Chip8Machine> {
        let mut chip_machine = Chip8Machine {
            memory: [0; 4096],
            program_counter: PROG_STARTING_ADDRESS,
            index_register: 0,
            stack: vec![],
            delay_timer: 0,
            sound_timer: 0,
            registers: [0; 16],
            current_input: None,
            display: [false; SCREEN_SIZE],
            original_behaviour,
            rand: rand::rng(),
        };
        chip_machine.memory[(FONT_SET_STARTING_ADDRESS as usize)..0x0A0].copy_from_slice(&FONT_SET); // Copy font set into memory

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
                0x0 => Ok(OpCode::SetRegisterToRegister { x, y }),
                0x1 => Ok(OpCode::BinaryOr { x, y }),
                0x2 => Ok(OpCode::BinaryAnd { x, y }),
                0x3 => Ok(OpCode::LogicalXor { x, y }),
                0x4 => Ok(OpCode::AddRegisterToRegister { x, y }),
                0x5 => Ok(OpCode::SubtractXY { x, y }),
                0x6 => Ok(OpCode::ShiftRight { x, y }),
                0x7 => Ok(OpCode::SubtractYX { x, y }),
                0xE => Ok(OpCode::ShiftLeft { x, y }),
                _ => panic!("Unrecognised 8XYN opcode with nibble {n}"),
            },
            0x9 => Ok(OpCode::SkipIfRegisterNotEqual { x, y }),
            0xA => Ok(OpCode::SetIndexRegister { nnn }),
            0xB => Ok(OpCode::JumpWithOffset { x, nnn }),
            0xC => Ok(OpCode::Random { x, nn }),
            0xD => Ok(OpCode::Draw { x, y, n }),
            0xE => match nn {
                0x9E => Ok(OpCode::SkipIfKey { x }),
                0xA1 => Ok(OpCode::SkipIfNotKey { x }),
                _ => panic!("Unrecognised EXNN opcode with NN {nn}"),
            },
            0xF => match nn {
                0x07 => Ok(OpCode::SetRegisterToDelayTimer { x }),
                0x0A => Ok(OpCode::GetKey { x }),
                0x15 => Ok(OpCode::SetDelayTimerToRegister { x }),
                0x18 => Ok(OpCode::SetSoundTimerToRegister { x }),
                0x1E => Ok(OpCode::AddRegisterToIndex { x }),
                0x29 => Ok(OpCode::FontCharacter { x }),
                0x33 => Ok(OpCode::BinaryCodedDecimalConversion { x }),
                0x55 => Ok(OpCode::StoreRegistersToMemory { x }),
                0x65 => Ok(OpCode::LoadRegistersFromMemory { x }),
                _ => panic!("Unrecognised FXNN opcode with NN {nn}"),
            },
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
            OpCode::PopSubroutine => {
                let prev = self
                    .stack
                    .pop()
                    .expect("No previous address on stack when popping subroutine!");
                self.program_counter = prev;
            }
            OpCode::CallSubroutine { nnn } => {
                self.stack.push(self.program_counter);
                self.program_counter = nnn;
            }
            OpCode::SkipIfEqual { x, nn } => {
                if self.registers[x as usize] == nn {
                    self.program_counter += 2;
                }
            }
            OpCode::SkipIfNotEqual { x, nn } => {
                if self.registers[x as usize] != nn {
                    self.program_counter += 2;
                }
            }
            OpCode::SkipIfRegisterEqual { x, y } => {
                if self.registers[x as usize] == self.registers[y as usize] {
                    self.program_counter += 2;
                }
            }
            OpCode::SkipIfRegisterNotEqual { x, y } => {
                if self.registers[x as usize] != self.registers[y as usize] {
                    self.program_counter += 2;
                }
            }
            OpCode::SetRegister { x, nn } => {
                self.registers[x as usize] = nn;
            }
            OpCode::AddValueToRegister { x, nn } => {
                self.registers[x as usize] += nn;
            }
            OpCode::SetRegisterToRegister { x, y } => {
                self.registers[x as usize] = self.registers[y as usize];
            }
            OpCode::BinaryOr { x, y } => {
                self.registers[x as usize] =
                    self.registers[x as usize] | self.registers[y as usize];
            }
            OpCode::BinaryAnd { x, y } => {
                self.registers[x as usize] =
                    self.registers[x as usize] & self.registers[y as usize];
            }
            OpCode::LogicalXor { x, y } => {
                self.registers[x as usize] =
                    self.registers[x as usize] ^ self.registers[y as usize];
            }
            OpCode::AddRegisterToRegister { x, y } => {
                self.registers[x as usize] += self.registers[y as usize];
                // If this overflows, set VF to 1
                if self.registers[x as usize] < self.registers[y as usize] {
                    self.registers[CARRY_FLAG_REG] = 1;
                } else {
                    self.registers[CARRY_FLAG_REG] = 0;
                }
            }
            OpCode::SubtractXY { x, y } => {
                if self.registers[x as usize] >= self.registers[y as usize] {
                    self.registers[CARRY_FLAG_REG] = 1;
                } else {
                    self.registers[CARRY_FLAG_REG] = 0;
                }
                self.registers[x as usize] =
                    self.registers[x as usize] - self.registers[y as usize];
            }
            OpCode::SubtractYX { x, y } => {
                if self.registers[y as usize] >= self.registers[x as usize] {
                    self.registers[CARRY_FLAG_REG] = 1;
                } else {
                    self.registers[CARRY_FLAG_REG] = 0;
                }
                self.registers[x as usize] =
                    self.registers[y as usize] - self.registers[x as usize];
            }
            OpCode::ShiftRight { x, y } => {
                if self.original_behaviour {
                    self.registers[x as usize] = self.registers[y as usize];
                }
                // Set carry flag to bit about to be shifted out
                self.registers[CARRY_FLAG_REG] = self.registers[x as usize] & 0b_00000001;
                self.registers[x as usize] = self.registers[x as usize] >> 1;
            }
            OpCode::ShiftLeft { x, y } => {
                if self.original_behaviour {
                    self.registers[x as usize] = self.registers[y as usize];
                }
                // Set carry flag to bit about to be shifted out
                if self.registers[x as usize] & 0b_10000000 == 0b_10000000 {
                    self.registers[CARRY_FLAG_REG] = 1;
                } else {
                    self.registers[CARRY_FLAG_REG] = 0;
                }
                self.registers[x as usize] = self.registers[x as usize] << 1;
            }
            OpCode::SetIndexRegister { nnn } => {
                self.index_register = nnn;
            }
            OpCode::JumpWithOffset { x, nnn } => {
                // TODO: Maybe add config for modern behaviour
                if self.original_behaviour {
                    self.program_counter = self.registers[0x0] as u16 + nnn;
                } else {
                    self.program_counter = self.registers[x as usize] as u16 + nnn;
                }
            }
            OpCode::Random { x, nn } => {
                let random_num = self.rand.random::<u8>();
                self.registers[x as usize] = random_num & nn;
            }
            OpCode::Draw { x, y, n } => {
                let x_coord = self.registers[x as usize] % PIXEL_COLUMNS as u8;
                let y_coord = self.registers[y as usize] % PIXEL_ROWS as u8;
                self.registers[CARRY_FLAG_REG] = 0;

                // Draw the sprite, setting 0xF to 1 if anything was turned off by this
                for row in 0..n {
                    let pixel_y = y_coord + row;
                    if pixel_y as usize >= PIXEL_ROWS {
                        break;
                    }

                    let sprite_row = self.memory[(self.index_register + row as u16) as usize];
                    for bit in 0..8 {
                        let mask: u8 = 0b_10000000 >> bit;
                        let sprite_pixel_on: bool = (sprite_row | mask) == sprite_row;

                        let pixel_x = x_coord + bit as u8;
                        if pixel_x >= PIXEL_COLUMNS as u8 {
                            continue; // Stop if we're about to go off-screen
                        }
                        let display_pixel_index = pixel_y as usize * PIXEL_COLUMNS + pixel_x as usize;
                        let display_pixel_on = self.display[display_pixel_index];

                        if sprite_pixel_on && display_pixel_on {
                            self.registers[CARRY_FLAG_REG] = 1;
                            self.display[display_pixel_index] = false;
                        } else if sprite_pixel_on {
                            self.display[display_pixel_index] = true;
                        }
                    }
                }
            }
            OpCode::SkipIfKey { x } => match self.current_input {
                None => {}
                Some(key) => {
                    if key == self.registers[x as usize] {
                        self.program_counter += 2;
                    }
                }
            },
            OpCode::SkipIfNotKey { x } => match self.current_input {
                None => {
                    self.program_counter += 2;
                }
                Some(key) => {
                    if key != self.registers[x as usize] {
                        self.program_counter += 2;
                    }
                }
            },
            OpCode::SetRegisterToDelayTimer { x } => {
                self.registers[x as usize] = self.delay_timer;
            }
            OpCode::SetDelayTimerToRegister { x } => {
                self.delay_timer = self.registers[x as usize];
            }
            OpCode::SetSoundTimerToRegister { x } => {
                self.sound_timer = self.registers[x as usize];
            }
            OpCode::AddRegisterToIndex { x } => {
                self.index_register += self.registers[x as usize] as u16;
                if self.index_register > 0xFFF {
                    self.registers[CARRY_FLAG_REG] = 1;
                }
            }
            OpCode::GetKey { x } => {
                match self.current_input {
                    None => {
                        self.program_counter -= 2;
                    }
                    Some(key) => {
                        if key != self.registers[x as usize] {
                            self.program_counter -= 2; // Block until the key is pressed
                        }
                    }
                }
            }
            OpCode::FontCharacter { x } => {
                self.index_register = FONT_SET_STARTING_ADDRESS + (self.registers[x as usize] as u16) * 5;
            }
            OpCode::BinaryCodedDecimalConversion { x } => {
                let value = self.registers[x as usize];
                let first_digit = value / 100;
                let second_digit = (value % 100) / 10;
                let third_digit = value % 10;
                self.memory[self.index_register as usize] = first_digit;
                self.memory[self.index_register as usize + 1] = second_digit;
                self.memory[self.index_register as usize + 2] = third_digit;
            }
            OpCode::StoreRegistersToMemory { x } => {
                if self.original_behaviour {
                    for register in 0..=x {
                        self.memory[self.index_register as usize] =
                            self.registers[register as usize];
                        self.index_register += 1;
                    }
                } else {
                    for register in 0..=x  {
                        self.memory[(self.index_register + register as u16) as usize] =
                            self.registers[register as usize];
                    }
                }
            }
            OpCode::LoadRegistersFromMemory { x } => {
                if self.original_behaviour {
                    for register in 0..=x {
                        self.registers[register as usize] =
                            self.memory[self.index_register as usize];
                        self.index_register += 1;
                    }
                } else {
                    for register in 0..=x {
                        self.registers[register as usize] =
                            self.memory[(self.index_register + register as u16) as usize];
                    }
                }
            }
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

    pub fn get_playing_sound(&self) -> bool {
        self.sound_timer > 0
    }
}
