mod keyboard;
mod machine;
mod memory;
mod opcodes;
mod periphery;
mod processor;
mod terminal;
mod timer;

use std::{
    cell::RefCell,
    collections::HashMap,
    env,
    error::Error,
    io,
    path::Path,
    rc::Rc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use keyboard::{KeyState, Keyboard};
use machine::Machine;
use num_format::{CustomFormat, ToFormattedString};
use opcodes::Opcode;
use periphery::Periphery;
use processor::Processor;
use raylib::prelude::*;
use serde::{Deserialize, Serialize};
use timer::Timer;

use crate::opcodes::OpcodeDescription;

pub struct Size2D {
    width: i32,
    height: i32,
}

pub const SCREEN_SIZE: Size2D = Size2D {
    width: 1600,
    height: 900,
};

pub const OPCODE_LENGTH: usize = 16;

pub const fn static_assert(condition: bool) {
    assert!(condition);
}

pub const TARGET_FPS: u64 = 60;

pub type Instruction = u64;
pub type Word = u32;
pub type HalfWord = u16;
pub type Address = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Register(pub u8);

impl From<u8> for Register {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

const _: () = static_assert(HalfWord::SIZE * 2 == Word::SIZE);

pub trait AsHalfWords {
    fn as_half_words(&self) -> (HalfWord, HalfWord);
}

impl AsHalfWords for Word {
    fn as_half_words(&self) -> (HalfWord, HalfWord) {
        (
            (self >> (8 * HalfWord::SIZE)) as HalfWord,
            *self as HalfWord,
        )
    }
}

pub trait AsWords {
    fn as_words(&self) -> (Word, Word);
}

impl AsWords for Instruction {
    fn as_words(&self) -> (Word, Word) {
        ((self >> (Word::SIZE * 8)) as Word, *self as Word)
    }
}

pub trait Size: Sized {
    const SIZE: usize = std::mem::size_of::<Self>();
}

impl Size for Instruction {}
impl Size for Word {}
impl Size for HalfWord {}

fn main() -> Result<(), Box<dyn Error>> {
    let rom_filename = env::args()
        .nth(1)
        .ok_or("Please specify the ROM to be loaded as a command line argument.")?;

    match env::args().nth(1).unwrap().as_str() {
        "emit" => {
            if env::args().len() != 3 {
                return Err("Please specify an output filename".into());
            }
            save_opcodes_as_machine_code(
                &[
                    Opcode::MoveRegisterImmediate {
                        register: 10.into(),
                        immediate: 1000,
                    },
                    Opcode::MoveRegisterImmediate {
                        register: 11.into(),
                        immediate: 10,
                    },
                    Opcode::PollTime {
                        high: 0xCC.into(),
                        low: 1.into(),
                    },
                    Opcode::DivmodTargetModLhsRhs {
                        result: 2.into(),
                        remainder: 0xDD.into(),
                        lhs: 1.into(),
                        rhs: 10.into(),
                    },
                    Opcode::DivmodTargetModLhsRhs {
                        result: 0xEE.into(),
                        remainder: 3.into(),
                        lhs: 2.into(),
                        rhs: 11.into(),
                    },
                    Opcode::AddTargetSourceImmediate {
                        target: 4.into(),
                        source: 3.into(),
                        immediate: b'0'.into(),
                    },
                    Opcode::MoveAddressRegister {
                        register: 4.into(),
                        address: 0x0,
                    },
                    Opcode::JumpAddress {
                        address: Processor::ENTRY_POINT + 2 * Instruction::SIZE as Address,
                    },
                ],
                &env::args().nth(2).unwrap(),
            )?;
            return Ok(());
        }
        "json" => {
            if env::args().len() != 3 {
                return Err("Please specify an output filename".into());
            }
            #[derive(Serialize)]
            struct JsonInfo {
                opcodes: HashMap<&'static str, OpcodeDescription>,
                constants: HashMap<&'static str, u64>,
            }
            let json_info = JsonInfo {
                opcodes: Opcode::as_hashmap(),
                constants: HashMap::from([
                    ("ENTRY_POINT", Processor::ENTRY_POINT as _),
                    ("NUM_REGISTERS", Processor::NUM_REGISTERS as _),
                    ("CYCLE_COUNT_HIGH", Processor::CYCLE_COUNT_HIGH.0 as _),
                    ("CYCLE_COUNT_LOW", Processor::CYCLE_COUNT_LOW.0 as _),
                    ("FLAGS", Processor::FLAGS.0 as _),
                    ("INSTRUCTION_POINTER", Processor::INSTRUCTION_POINTER.0 as _),
                    ("STACK_POINTER", Processor::STACK_POINTER.0 as _),
                    ("STACK_START", Processor::STACK_START as _),
                    ("STACK_SIZE", Processor::STACK_SIZE as _),
                ]),
            };
            let json_string = serde_json::to_string_pretty(&json_info).unwrap();
            std::fs::write(&env::args().nth(2).unwrap(), &json_string)?;
            return Ok(());
        }
        _ => {}
    }

    if env::args().nth(1).unwrap() == "emit" {}

    let (raylib_handle, thread) = raylib::init()
        .size(SCREEN_SIZE.width, SCREEN_SIZE.height)
        .title("Backseater")
        .build();
    let raylib_handle = Rc::new(RefCell::new(raylib_handle));
    let raylib_handle_copy = Rc::clone(&raylib_handle);
    let periphery = Periphery {
        timer: Timer::new(ms_since_epoch),
        keyboard: Keyboard::new(Box::new(move |key| {
            match raylib_handle_copy.borrow().is_key_down(
                raylib::input::key_from_i32(key.try_into().expect("keycode out of range"))
                    .expect("invalid keycode"),
            ) {
                true => KeyState::Down,
                false => KeyState::Up,
            }
        })),
    };
    let mut machine = Machine::new(periphery);
    load_rom(&mut machine, rom_filename)?;

    let font = raylib_handle
        .borrow_mut()
        .load_font(&thread, "./resources/CozetteVector.ttf")?;
    let mut is_halted = false;

    let mut time_measurements = TimeMeasurements {
        next_render_time: ms_since_epoch(),
        last_cycle_count: 0,
        last_render_time: 0,
        clock_frequency_accumulator: 0,
        next_clock_frequency_render: ms_since_epoch() + 1000,
        num_clock_frequency_accumulations: 0,
        clock_frequency_average: 0,
    };

    let custom_number_format = CustomFormat::builder().separator(" ").build()?;

    while !raylib_handle.borrow().window_should_close() {
        let current_time = ms_since_epoch();
        render_if_needed(
            current_time,
            &mut time_measurements,
            &mut raylib_handle.borrow_mut(),
            &thread,
            &machine,
            &font,
            &custom_number_format,
        );

        let num_cycles = match (
            time_measurements.clock_frequency_average,
            current_time > time_measurements.next_render_time,
        ) {
            (_, true) => {
                time_measurements.next_render_time = current_time;
                0
            }
            (0, false) => 10_000,
            (_, false) => {
                let remaining_ms_until_next_render =
                    time_measurements.next_render_time - current_time;
                let cycle_duration = 1000.0 / time_measurements.clock_frequency_average as f64;
                (remaining_ms_until_next_render as f64 / cycle_duration - 10.0) as u64
            }
        };

        for _ in 0..num_cycles {
            execute_next_instruction(&mut is_halted, &mut machine);
        }
    }
    Ok(())
}

fn load_rom(machine: &mut Machine, filename: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    let buffer = std::fs::read(filename)?;
    if buffer.len() % Instruction::SIZE != 0 {
        return Err(format!("Filesize must be divisible by {}", Instruction::SIZE).into());
    }
    let iterator = buffer
        .chunks_exact(Instruction::SIZE)
        .map(|slice| Instruction::from_be_bytes(slice.try_into().unwrap()));
    for (instruction, address) in
        iterator.zip((Processor::ENTRY_POINT..).step_by(Instruction::SIZE))
    {
        machine.memory.write_opcode(
            address,
            instruction.try_into().expect("Invalid instruction"),
        );
    }
    Ok(())
}

fn duration_since_epoch() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
}

fn ms_since_epoch() -> u64 {
    let since_the_epoch = duration_since_epoch();
    since_the_epoch.as_secs() * 1000 + since_the_epoch.subsec_nanos() as u64 / 1_000_000
}

fn execute_next_instruction(is_halted: &mut bool, machine: &mut Machine) {
    match (*is_halted, machine.is_halted()) {
        (false, true) => {
            *is_halted = true;
            println!("HALT AND CATCH FIRE");
        }
        (false, false) => {
            machine.execute_next_instruction();
        }
        (_, _) => {}
    }
}

fn save_opcodes_as_machine_code(instructions: &[Opcode], filename: &str) -> io::Result<()> {
    let file_contents: Vec<_> = instructions
        .iter()
        .map(|opcode| opcode.as_instruction())
        .flat_map(|instruction| instruction.to_be_bytes())
        .collect();
    std::fs::write(filename, &file_contents)
}

struct TimeMeasurements {
    next_render_time: u64,
    last_cycle_count: u64,
    last_render_time: u64,
    clock_frequency_accumulator: u64,
    next_clock_frequency_render: u64,
    num_clock_frequency_accumulations: u64,
    clock_frequency_average: u64,
}

fn render_if_needed(
    current_time: u64,
    time_measurements: &mut TimeMeasurements,
    raylib_handle: &mut RaylibHandle,
    thread: &RaylibThread,
    machine: &Machine,
    font: &Font,
    custom_number_format: &CustomFormat,
) {
    if current_time >= time_measurements.next_render_time {
        time_measurements.next_render_time += 1000 / TARGET_FPS;

        let mut draw_handle = raylib_handle.begin_drawing(thread);
        render(&mut draw_handle, machine, font);

        let current_cycle_count = machine.processor.get_cycle_count();
        if current_time != time_measurements.last_render_time {
            calculate_clock_frequency(current_time, time_measurements, current_cycle_count);
            draw_clock_frequency(
                time_measurements,
                custom_number_format,
                &mut draw_handle,
                font,
            );
        }
        time_measurements.last_render_time = current_time;
        time_measurements.last_cycle_count = current_cycle_count;
    }
}

fn render(draw_handle: &mut RaylibDrawHandle, machine: &Machine, font: &Font) {
    draw_handle.clear_background(Color::BLACK);
    machine.render(draw_handle, font);
    draw_handle.draw_fps(SCREEN_SIZE.width - 150, 10);
}

fn calculate_clock_frequency(
    current_time: u64,
    time_measurements: &mut TimeMeasurements,
    current_cycle_count: u64,
) {
    let time_since_last_render = current_time - time_measurements.last_render_time;
    let cycles_since_last_render = current_cycle_count - time_measurements.last_cycle_count;
    let clock_frequency = 1000 * cycles_since_last_render / time_since_last_render;
    time_measurements.clock_frequency_accumulator += clock_frequency;
    time_measurements.num_clock_frequency_accumulations += 1;
    if current_time >= time_measurements.next_clock_frequency_render {
        time_measurements.clock_frequency_average = time_measurements.clock_frequency_accumulator
            / time_measurements.num_clock_frequency_accumulations;
        time_measurements.next_clock_frequency_render = current_time + 1000;
        time_measurements.clock_frequency_accumulator = 0;
        time_measurements.num_clock_frequency_accumulations = 0;
    }
}

fn draw_clock_frequency(
    time_measurements: &TimeMeasurements,
    custom_number_format: &CustomFormat,
    draw_handle: &mut RaylibDrawHandle,
    font: &Font,
) {
    draw_handle.draw_text_ex(
        font,
        &*format!(
            "{} kHz",
            (time_measurements.clock_frequency_average / 1000)
                .to_formatted_string(custom_number_format)
        ),
        Vector2::new(SCREEN_SIZE.width as f32 - 200.0, 100.0),
        30.0,
        1.0,
        Color::WHITE,
    );
}
