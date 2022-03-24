mod font;
mod instruction;
mod test;

use crate::vm::{Display, Keys};
use anyhow::Result;
use instruction::{decode, Instruction};
use rand::random;
use std::num::Wrapping;
use std::time::Duration;
pub struct Cpu {
    memory: [u8; 4096],
    pc: u16,
    index: u16,
    stack: Vec<u16>,
    delay_timer: u8,
    sound_timer: u8,
    registers: [u8; 16],
    speed: Duration,
}

impl crate::vm::Chip8Cpu for Cpu {
    //this should execute in the time 1/speed
    fn step(&mut self, display: &mut Display, keys: &Keys) {
        let opcode = self.fetch();
        let instruction = decode(opcode);

        dbg!(&instruction);
        self.exectute(instruction, display, keys);
    }

    fn speed(&self) -> Duration {
        self.speed
    }
}

impl Cpu {
    pub fn new(speed: u32) -> Self {
        let mut memory = [0_u8; 4096];

        //font is 80 bytes, should lie at 0x50
        memory[0x50..0xA0].copy_from_slice(&font::FONT);

        Cpu {
            memory: [0; 4096],
            pc: 0,
            index: 0,
            delay_timer: 0,
            sound_timer: 0,
            stack: Vec::new(),
            registers: [0; 16],
            speed: Duration::from_secs_f64(1_f64 / speed as f64),
        }
    }

    pub fn load(mut self, filename: &str) -> Result<Self> {
        let program = std::fs::read(filename)?;
        self.memory[0x200..(0x200 + program.len())].copy_from_slice(&program);
        self.pc = 0x200;
        Ok(self)
    }

    fn fetch(&mut self) -> u16 {
        let instruction = u16::from_be_bytes([
            self.memory[self.pc as usize],
            self.memory[(self.pc + 1) as usize],
        ]);
        self.inc_pc();
        instruction
    }

    fn exectute(&mut self, instruction: Instruction, display: &mut Display, keys: &Keys) {
        match instruction {
            Instruction::Nop => (),
            Instruction::Cls => display.copy_from_slice(&[[0; 64]; 32]),
            Instruction::Rts => {
                self.pc = self.stack.pop().unwrap_or(0);
            }
            Instruction::Jmp(addr) => {
                self.pc = addr;
            }
            Instruction::Call(addr) => {
                self.stack.push(self.pc);
                self.pc = addr;
            }
            Instruction::Loadr(r, byte) => {
                self.registers[r as usize] = byte;
            }
            Instruction::Add(r, byte) => self.registers[r as usize] = self.registers[r as usize].wrapping_add(byte),
            Instruction::Loadi(nnn) => {
                self.index = nnn;
            }
            Instruction::Draw(rx, ry, n) => {
                let range = (self.index as usize)..((self.index + n as u16) as usize);
                let sprite = &self.memory[range];
                let x = self.registers[rx as usize] & 63;
                let y = self.registers[ry as usize] & 31;
                self.registers[0xf] = 0;
                dbg!(&sprite);
                for (i, row) in sprite.iter().enumerate() {
                    for (j, sprite_px) in (0..8).zip(PixIterator::new(row)) {
                        let display_px = display[y as usize + i][x as usize + j];
                        dbg!(display_px, sprite_px, x, y);
                        //set vf on collide
                        if display_px == 1 && sprite_px == 1 {
                            self.registers[0xf] = 1;
                        }
                        //xor onto display
                        display[y as usize + i][x as usize + j] ^= sprite_px;

                        //are we at the right edge of the screen?
                        if x == 63 {
                            break;
                        }
                    }
                    // are we at the bottom of the screen?
                    if y == 31 {
                        break;
                    }
                }
            }
            Instruction::Ske(r, byte) => {
                if self.registers[r as usize] == byte {
                    self.inc_pc();
                }
            }
            Instruction::Skne(r, byte) => {
                if self.registers[r as usize] != byte {
                    self.inc_pc();
                }
            }
            Instruction::Skre(r1, r2) => {
                if self.registers[r1 as usize] == self.registers[r2 as usize] {
                    self.inc_pc();
                }
            }
            Instruction::Move(r1, r2) => self.registers[r1 as usize] = self.registers[r2 as usize],
            Instruction::Or(r1, r2) => self.registers[r1 as usize] |= self.registers[r2 as usize],
            Instruction::And(r1, r2) => self.registers[r1 as usize] &= self.registers[r2 as usize],
            Instruction::Xor(r1, r2) => self.registers[r1 as usize] ^= self.registers[r2 as usize],
            Instruction::Addr(r1, r2) => {
                let (result,overflow) = self.registers[r1 as usize].overflowing_add(self.registers[r2 as usize]);
                self.registers[r1 as usize] = result;
                self.registers[0xf] = overflow.into();
            }
            Instruction::Sub(r1, r2) => {
                let (result,overflow) = self.registers[r1 as usize].overflowing_sub(self.registers[r2 as usize]);
                self.registers[r1 as usize] = result;
                self.registers[0xf] = overflow.into();
            }
            Instruction::Shr(r1, _) => {
                //r2 is ignored
                self.registers[0xf] = 1 & self.registers[r1 as usize];
                self.registers[r1 as usize] >>= 1;
            }
            Instruction::Ssub(r1, r2) => {
                let (result,overflow) = self.registers[r2 as usize].overflowing_sub(self.registers[r1 as usize]);
                self.registers[r1 as usize] = result;
                self.registers[0xf] = overflow.into();
            }
            Instruction::Shl(r1, _) => {
                //r2 is ignored
                self.registers[0xf] = 0x80 & &self.registers[r1 as usize];
                self.registers[r1 as usize] <<= 1;
            }
            Instruction::Skrne(r1, r2) => {
                if self.registers[r1 as usize] != self.registers[r2 as usize] {
                    self.inc_pc();
                }
            }
            Instruction::Jumpi(nnn) => self.pc = (nnn + self.registers[0] as u16) & 0xfff, //u12 wrap
            Instruction::Rand(r, byte) => self.registers[r as usize] = random::<u8>() & byte,
            Instruction::Skp(r) => {
                if keys[r as usize] {
                    self.inc_pc()
                }
            }
            Instruction::Sknp(r) => {
                if !keys[r as usize] {
                    self.inc_pc()
                }
            }
            Instruction::Moved(r) => self.registers[r as usize] = self.delay_timer,
            Instruction::Key(_) => panic!("I haven't done this instruction because it blocks, making timing super hard. If you're trying to play a ROM that depends on this, simply don't."),
            Instruction::Loadd(r) => self.delay_timer = self.registers[r as usize],
            Instruction::Loads(r) => self.sound_timer = self.registers[r as usize],
            Instruction::Addi(r) => {
                //weird wrapping arithmetic, u16+u8 but has to wrap to a u12
                self.index += (self.registers[r as usize] as u16) & 0xfff;
            }
            Instruction::Ldfnt(r) => {
                //font starts at 0x50 in memory
                self.index = 0x50 + (self.registers[r as usize] * 5) as u16;
            }
            Instruction::Bcd(r) => {
                //binary encoded decimal conversion
                let val = self.registers[r as usize];
                self.memory[self.index as usize] = val / 100;
                self.memory[self.index as usize + 1] = val % 100 / 10;
                self.memory[self.index as usize + 2] = val % 10;
            }
            Instruction::Store(r) => {
                let addrs = (self.index as usize)..(self.index as usize + r as usize);
                self.memory[addrs].copy_from_slice(&self.registers[..(r as usize)]);
            }
            Instruction::Load(r) => {
                let addrs = (self.index as usize)..(self.index as usize + r as usize);
                self.registers[..(r as usize)].copy_from_slice(&self.memory[addrs]);
            }
        }
    }

    //helpers for stuff involving wrapping
    //wrapping pc incremement so we dont forget to do it anywhere
    fn inc_pc(&mut self) {
        self.pc += 2;
        self.pc &= 0xfff;
    }
}
//helpers here

//break a u16 into its nibbles
fn nibbles(n: u16) -> (u8, u8, u8, u8) {
    let n3 = (n >> 12) as u8;
    let n2 = ((n >> 8) & 0b1111) as u8;
    let n1 = ((n >> 4) & 0b1111) as u8;
    let n0 = (n & 0b1111) as u8;
    (n3, n2, n1, n0)
}

//get the lower 12 bits of a u16
fn twelvebit(n: u16) -> u16 {
    n & 0xfff
}

//get the lower 8 bits of a u16
fn eightbit(n: u16) -> u8 {
    (n & 0xff) as u8
}

//helpers
//an iterator over the bits of a byte
struct PixIterator {
    byte: u8,
    idx: u8,
}

impl PixIterator {
    pub fn new(byte: &u8) -> Self {
        Self {
            byte: *byte,
            idx: 0,
        }
    }
}

impl Iterator for PixIterator {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < 8 {
            let bit = self.byte >> (7 - self.idx) & 1;
            self.idx += 1;
            assert!(bit == 1 || bit == 0);
            Some(bit)
        } else {
            None
        }
    }
}
