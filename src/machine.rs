use crate::{memory::Memory, opcodes::Opcode, processor::Processor, terminal, Instruction};
use raylib::prelude::*;

pub struct Machine {
    pub memory: Memory,
    pub processor: Processor,
}

impl Machine {
    pub fn new() -> Self {
        Self {
            memory: Memory::new(),
            processor: Processor::new(),
        }
    }

    pub fn render(&self, draw_handle: &mut RaylibDrawHandle, font: &Font) {
        terminal::render(&self.memory, draw_handle, Vector2::zero(), font, 20.0);
    }

    pub fn make_tick(&mut self) {
        self.processor.make_tick(&mut self.memory);
    }

    #[must_use = "Am I a joke to you?"]
    pub fn is_halted(&self) -> bool {
        let opcode = self.read_opcode_at_instruction_pointer();
        matches!(opcode, Ok(Opcode::HaltAndCatchFire {}))
    }

    fn read_opcode_at_instruction_pointer(
        &self,
    ) -> Result<Opcode, <Opcode as TryFrom<Instruction>>::Error> {
        self.memory
            .read_opcode(self.processor.registers[Processor::INSTRUCTION_POINTER])
    }
}

#[cfg(test)]
mod tests {
    use crate::processor::Flag;
    use crate::{
        opcodes::Opcode::{self, *},
        Register,
    };
    use crate::{Address, Instruction, Size, Word};

    use super::*;

    macro_rules! opcodes_to_machine {
        () => {
            Machine::new()
        };
        ($opcodes:expr) => {
            create_machine_with_opcodes($opcodes)
        };
    }

    macro_rules! create_test {
        (
            $test_name:ident,
            $( setup = { $($setup_tokens:tt)+ }, )?
            $( opcodes = $opcodes:expr, )?
            $( registers_pre = [$( $register_pre_value:expr => $register_pre:expr ),+], )?
            $( flags_pre = [ $( $flag_pre_value:expr => $flag_pre:ident ),+ ],)?
            $( memory_pre = [$( $memory_pre_value:expr => $memory_pre_address:expr ),+], )?
            $( registers_post = [$( ($register_post:expr, $register_post_value:expr) ),+], )?
            $( memory_post = [$( ( $memory_post_address:expr, $memory_post_value:expr ) ),+], )?
            $( flags_post = [ $( ( $flag_post:ident, $flag_post_value:expr ) ),+], )?
            $( eq_asserts = [ $( ( $eq_assert_lhs:expr, $eq_assert_rhs:expr ) ),+ ], )?
        ) => {
            #[test]
            fn $test_name() {
                $(
                    $(
                        $setup_tokens
                    )+
                )?
                let mut machine = opcodes_to_machine!($( $opcodes )?);
                $(
                    $(
                        machine.processor.registers[$register_pre.into()] = $register_pre_value;
                    )+
                )?
                $(
                    $(
                        machine.processor.set_flag(Flag::$flag_pre, $flag_pre_value);
                    )+
                )?
                $(
                    $(
                        machine.memory.write_data($memory_pre_address, $memory_pre_value);
                    )+
                )?
                $(
                    for _ in 0..$opcodes.len() {
                        machine.make_tick();
                    }
                )?
                $(
                    $(
                        assert_eq!(machine.processor.registers[$register_post], $register_post_value);
                    )+
                )?
                $(
                    $(
                        assert_eq!(machine.memory.read_data($memory_post_address), $memory_post_value);
                    )+
                )?
                $(
                    $(
                        assert_eq!(machine.processor.get_flag(Flag::$flag_post), $flag_post_value);
                    )+
                )?
                $(
                    $(
                        assert_eq!($eq_assert_lhs, $eq_assert_rhs);
                    )+
                )?
            }
        };
    }

    fn create_machine_with_data_at(address: Address, data: Word) -> Machine {
        let mut machine = Machine::new();
        machine.memory.write_data(address, data);
        machine
    }

    fn create_machine_with_opcodes(opcodes: &[Opcode]) -> Machine {
        let mut machine = Machine::new();
        for (&opcode, address) in opcodes
            .iter()
            .zip((Processor::ENTRY_POINT..).step_by(Instruction::SIZE))
        {
            machine.memory.write_opcode(address, opcode);
        }
        machine
    }

    fn execute_instruction_with_machine(mut machine: Machine, opcode: Opcode) -> Machine {
        let instruction_pointer = machine.processor.registers[Processor::INSTRUCTION_POINTER];
        machine.memory.write_opcode(instruction_pointer, opcode);
        machine.processor.make_tick(&mut machine.memory);
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            instruction_pointer + Instruction::SIZE as u32
        );
        machine
    }

    fn execute_instruction(opcode: Opcode) -> Machine {
        execute_instruction_with_machine(Machine::new(), opcode)
    }

    create_test!(
        make_tick_increases_instruction_pointer,
        opcodes = &[Opcode::MoveRegisterImmediate {
            register: 0.into(),
            immediate: 0
        }],
        registers_post = [(
            Processor::INSTRUCTION_POINTER,
            Processor::ENTRY_POINT + Instruction::SIZE as u32
        )],
    );

    create_test!(
        move_constant_into_register,
        setup = {
            let register = 0x0A.into();
            let value = 0xABCD_1234;
        },
        opcodes = &[MoveRegisterImmediate {
            register,
            immediate: value,
        }],
        registers_post = [(register, value)],
    );

    create_test!(
        move_from_address_into_register,
        setup = {
            let address = 0xF0;
            let data = 0xABCD_1234;
            let register = 0x0A.into();
        },
        opcodes = &[MoveRegisterAddress { register, address }],
        memory_pre = [data => address],
        registers_post = [(register, data)],
    );

    #[test]
    fn move_from_one_register_to_another() {
        let mut machine = Machine::new();
        let source = 0x5.into();
        let target = 0x0A.into();
        let data = 0xCAFE;
        machine.processor.registers[source] = data;
        let machine =
            execute_instruction_with_machine(machine, MoveTargetSource { target, source });
        assert_eq!(machine.processor.registers[target], data);
    }

    create_test!(
        move_from_register_into_memory,
        setup = {
            let register = Register(5);
            let data = 0xC0FFEE;
            let address = 0xF0;
        },
        opcodes = &[MoveAddressRegister { address, register }],
        registers_pre = [data => register],
        memory_post = [(address, data)],
    );

    create_test!(
        move_from_memory_addressed_by_register_into_another_register,
        setup = {
            let address = 0xF0;
            let data = 0xC0FFEE;
            let target = 0x0A.into();
            let pointer = 0x05.into();
        },
        opcodes = &[MoveTargetPointer { target, pointer }],
        registers_pre = [address => pointer],
        memory_pre = [data => address],
        registers_post = [(target, data)],
    );

    create_test!(
        move_from_memory_addressed_by_register_into_same_register,
        setup = {
            let address = 0xF0;
            let data = 0xC0FFEE;
            let register = 0x05.into();
        },
        opcodes = &[MoveTargetPointer {
            target: register,
            pointer: register,
        }],
        registers_pre = [address => register],
        memory_pre = [data => address],
        registers_post = [(register, data)],
    );

    create_test!(
        move_from_register_into_memory_addressed_by_another_register,
        setup = {
            let data = 0xC0FFEE;
            let address = 0xF0;
            let pointer = 0x0A.into();
            let source = 0x05.into();
        },
        opcodes = &[MovePointerSource { pointer, source }],
        registers_pre = [data => source, address => pointer],
        memory_post = [(address, data)],
    );

    create_test!(
        move_from_register_into_memory_addressed_by_same_register,
        setup = {
            let address = 0xF0;
            let register = 0x05.into();
        },
        opcodes = &[MovePointerSource { pointer: register, source: register }],
        registers_pre = [address => register],
        memory_post = [(address, address)],
    );

    create_test!(
        halt_and_catch_fire_prevents_further_instructions,
        setup = {
            let register = 0x05.into();
            let value = 0x0000_0042;
        },
        opcodes = &[
            HaltAndCatchFire {},
            MoveRegisterImmediate {
                register,
                immediate: value,
            }
        ],
        registers_post = [
            (Processor::INSTRUCTION_POINTER, Processor::ENTRY_POINT),
            (register, 0x0)
        ],
    );

    macro_rules! create_addition_test{
        (
            $test_name:ident,
            $lhs:expr,
            $rhs:expr,
            zero = $zero:literal,
            carry = $carry:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let lhs_register = 0x42.into();
                    let rhs_register = 0x43.into();
                    let target_register = 0x0A.into();
                    let lhs: Word = $lhs;
                    let rhs = $rhs;
                    let expected = lhs.wrapping_add(rhs);
                },
                opcodes = &[AddTargetLhsRhs {
                    target: target_register,
                    lhs: lhs_register,
                    rhs: rhs_register,
                }],
                registers_pre = [lhs => lhs_register, rhs => rhs_register],
                registers_post = [
                    (lhs_register, lhs),
                    (rhs_register, rhs),
                    (target_register, expected)
                ],
                flags_post = [(Zero, $zero), (Carry, $carry)],
            );
        };
    }

    create_addition_test!(
        add_two_values_with_no_flags_set,
        10,
        12,
        zero = false,
        carry = false
    );

    create_addition_test!(
        add_two_values_with_only_zero_flag_set,
        0,
        0,
        zero = true,
        carry = false
    );

    create_addition_test!(
        add_two_values_with_only_carry_flag_set,
        Word::MAX,
        5,
        zero = false,
        carry = true
    );

    create_addition_test!(
        add_two_values_with_both_zero_and_carry_flags_set,
        Word::MAX,
        1,
        zero = true,
        carry = true
    );

    macro_rules! create_subtraction_test{
        (
            $test_name:ident,
            $lhs:expr,
            $rhs:expr,
            zero = $zero:literal,
            carry = $carry:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let lhs_register = 0x42.into();
                    let rhs_register = 0x43.into();
                    let target_register = 0x0A.into();
                    let lhs: Word = $lhs;
                    let rhs = $rhs;
                    let expected = lhs.wrapping_sub(rhs);
                },
                opcodes = &[SubtractTargetLhsRhs {
                    target: target_register,
                    lhs: lhs_register,
                    rhs: rhs_register,
                }],
                registers_pre = [lhs => lhs_register, rhs => rhs_register],
                registers_post = [
                    (lhs_register, lhs),
                    (rhs_register, rhs),
                    (target_register, expected)
                ],
                flags_post = [(Zero, $zero), (Carry, $carry)],
            );
        };
    }

    create_subtraction_test!(
        subtract_two_values_with_no_flags_set,
        10,
        8,
        zero = false,
        carry = false
    );

    create_subtraction_test!(
        subtract_two_values_with_only_zero_flag_set,
        10,
        10,
        zero = true,
        carry = false
    );

    create_subtraction_test!(
        subtract_two_values_with_only_carry_flag_set,
        10,
        12,
        zero = false,
        carry = true
    );

    create_test!(
        subtract_two_values_with_carry_with_no_flags_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_register = 0x0A.into();
            let lhs: Word = 14;
            let rhs = 12;
            let expected = lhs.wrapping_sub(rhs + 1 /* carry */);
        },
        opcodes = &[SubtractWithCarryTargetLhsRhs {
            target: target_register,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        flags_pre = [true => Carry],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_register, expected)],
        flags_post = [(Zero, false), (Carry, false)],
    );

    create_test!(
        subtract_two_values_with_carry_with_zero_flag_set,
        setup = {
            let lhs_register = 0x42.into();
            let rhs_register = 0x43.into();
            let target_register = 0x0A.into();
            let lhs: Word = 14;
            let rhs = 13;
            let expected = lhs.wrapping_sub(rhs + 1 /* carry */);
        },
        opcodes = &[SubtractWithCarryTargetLhsRhs {
            target: target_register,
            lhs: lhs_register,
            rhs: rhs_register,
        }],
        registers_pre = [lhs => lhs_register, rhs => rhs_register],
        flags_pre = [true => Carry],
        registers_post = [(lhs_register, lhs), (rhs_register, rhs), (target_register, expected)],
        flags_post = [(Zero, true), (Carry, false)],
    );

    #[test]
    fn subtract_two_values_with_carry_with_both_carry_and_zero_flags_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0;
        let rhs = Word::MAX;
        let expected = lhs.wrapping_sub(rhs).wrapping_sub(1);
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            SubtractWithCarryTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn multiply_two_values_without_any_flags_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_high = 0x09.into();
        let target_low = 0x0A.into();
        let lhs: Word = 3;
        let rhs = 4;
        let expected = lhs * rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            MultiplyHighLowLhsRhs {
                high: target_high,
                low: target_low,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_high], 0);
        assert_eq!(machine.processor.registers[target_low], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn multiply_two_values_with_zero_flag_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_high = 0x09.into();
        let target_low = 0x0A.into();
        let lhs: Word = 3;
        let rhs = 0;
        let expected = lhs * rhs;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            MultiplyHighLowLhsRhs {
                high: target_high,
                low: target_low,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_high], 0);
        assert_eq!(machine.processor.registers[target_low], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn multiply_two_values_with_overflow() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_high = 0x09.into();
        let target_low = 0x0A.into();
        let lhs: Word = Word::MAX;
        let rhs = 5;
        let result = lhs as u64 * rhs as u64;
        let high_expected = (result >> 32) as u32;
        let low_expected = result as u32;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            MultiplyHighLowLhsRhs {
                high: target_high,
                low: target_low,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_high], high_expected);
        assert_eq!(machine.processor.registers[target_low], low_expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn multiply_two_values_with_overflow_and_zero_flag_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_high = 0x09.into();
        let target_low = 0x0A.into();
        let lhs: Word = 1 << (Word::BITS - 1);
        let rhs = 2;
        let result = lhs as u64 * rhs as u64;
        let high_expected = (result >> 32) as u32;
        let low_expected = result as u32;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            MultiplyHighLowLhsRhs {
                high: target_high,
                low: target_low,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_high], high_expected);
        assert_eq!(machine.processor.registers[target_low], low_expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn divmod_two_values_with_no_flags_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_quotient = 0x09.into();
        let target_remainder = 0x0A.into();
        let lhs: Word = 15;
        let rhs = 6;
        let expected_quotient = 2;
        let expected_remainder = 3;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            DivmodTargetModLhsRhs {
                result: target_quotient,
                remainder: target_remainder,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(
            machine.processor.registers[target_quotient],
            expected_quotient
        );
        assert_eq!(
            machine.processor.registers[target_remainder],
            expected_remainder
        );
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::DivideByZero));
    }

    #[test]
    fn divmod_two_values_with_zero_flag_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_quotient = 0x09.into();
        let target_remainder = 0x0A.into();
        let lhs: Word = 0;
        let rhs = 6;
        let expected_quotient = 0;
        let expected_remainder = 0;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            DivmodTargetModLhsRhs {
                result: target_quotient,
                remainder: target_remainder,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(
            machine.processor.registers[target_quotient],
            expected_quotient
        );
        assert_eq!(
            machine.processor.registers[target_remainder],
            expected_remainder
        );
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::DivideByZero));
    }

    #[test]
    fn divmod_two_values_divide_by_zero() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_quotient = 0x09.into();
        let target_remainder = 0x0A.into();
        let lhs: Word = 15;
        let rhs = 0;
        let expected_quotient = 0;
        let expected_remainder = 15;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            DivmodTargetModLhsRhs {
                result: target_quotient,
                remainder: target_remainder,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(
            machine.processor.registers[target_quotient],
            expected_quotient
        );
        assert_eq!(
            machine.processor.registers[target_remainder],
            expected_remainder
        );
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::DivideByZero));
    }

    #[test]
    fn bitwise_and_two_values_with_no_flags_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0b0110_1110_1001_1010_0110_1110_1001_1010;
        let rhs = 0b1011_1010_0101_1001_1011_1010_0101_1001;
        let expected = 0b0010_1010_0001_1000_0010_1010_0001_1000;
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            AndTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn bitwise_and_two_values_with_zero_flag_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0b0100_0100_1000_0110_0100_0100_1000_0010;
        let rhs = 0b1011_1010_0101_1001_1011_1010_0101_1001;
        let expected = 0;
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            AndTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn bitwise_or_two_values_with_no_flags_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0b0110_1110_1001_1010_0110_1110_1001_1010;
        let rhs = 0b1011_1010_0101_1001_1011_1010_0101_1001;
        let expected = 0b1111_1110_1101_1011_1111_1110_1101_1011;
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            OrTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn bitwise_or_two_values_with_zero_flag_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0;
        let rhs = 0;
        let expected = 0;
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            OrTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn bitwise_xor_two_values_with_no_flags_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0b0110_1110_1001_1010_0110_1110_1001_1010;
        let rhs = 0b1011_1010_0101_1001_1011_1010_0101_1001;
        let expected = 0b1101_0100_1100_0011_1101_0100_1100_0011;
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            XorTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn bitwise_xor_two_values_with_zero_flag_set() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs: Word = 0b1011_1010_1001_0010_0100_0100_1001_0010;
        let rhs = 0b1011_1010_1001_0010_0100_0100_1001_0010;
        let expected = 0;
        let mut machine = Machine::new();
        machine.processor.set_flag(Flag::Carry, true);
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            XorTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn bitwise_not_value_with_no_flags_set() {
        let mut machine = Machine::new();
        let source = 0x5.into();
        let target = 0x0A.into();
        let data = 0b0010_1010_0001_1000_0010_1010_0001_1000;
        let expected = 0b1101_0101_1110_0111_1101_0101_1110_0111;
        machine.processor.registers[source] = data;
        let machine = execute_instruction_with_machine(machine, NotTargetSource { target, source });
        assert_eq!(machine.processor.registers[target], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn bitwise_not_value_with_zero_flag_set() {
        let mut machine = Machine::new();
        let source = 0x5.into();
        let target = 0x0A.into();
        let data = 0xFFFFFFFF;
        let expected = 0;
        machine.processor.registers[source] = data;
        let machine = execute_instruction_with_machine(machine, NotTargetSource { target, source });
        assert_eq!(machine.processor.registers[target], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn left_shift_without_any_flags_set() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0b1;
        let rhs = 2;
        let expected = 0b100;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            LeftShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn left_shift_with_carry_flag_set() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0b11 << 30;
        let rhs = 1;
        let expected = 0b1 << 31;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            LeftShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn left_shift_with_carry_and_zero_flags_set() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0b1 << 31;
        let rhs = 1;
        let expected = 0;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            LeftShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn left_shift_way_too_far() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0xFFFF_FFFF;
        let rhs = 123;
        let expected = 0;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            LeftShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn left_shift_zero_way_too_far() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0;
        let rhs = 123;
        let expected = 0;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            LeftShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn right_shift_without_any_flags_set() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0b10;
        let rhs = 1;
        let expected = 0b1;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            RightShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn right_shift_with_carry_flag_set() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0b11;
        let rhs = 1;
        let expected = 0b1;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            RightShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn right_shift_with_zero_flag_set() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0b0;
        let rhs = 1;
        let expected = 0b0;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            RightShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn right_shift_with_carry_and_zero_flags_set() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0b1;
        let rhs = 1;
        let expected = 0b0;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            RightShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn right_shift_way_too_far() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0xFFFF_FFFF;
        let rhs = 123;
        let expected = 0b0;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            RightShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn right_shift_zero_way_too_far() {
        let mut machine = Machine::new();
        let lhs_register = 0x5.into();
        let rhs_register = 0x6.into();
        let target_register = 0x0A.into();
        let lhs = 0;
        let rhs = 123;
        let expected = 0b0;
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let machine = execute_instruction_with_machine(
            machine,
            RightShiftTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn add_immediate_with_no_flags_set() {
        let mut machine = Machine::new();
        let target_register = 0xAB.into();
        let source_register = 0x07.into();
        let immediate = 2;
        let source_value = 40;
        let expected_value = 42;
        machine.processor.registers[source_register] = source_value;
        let machine = execute_instruction_with_machine(
            machine,
            AddTargetSourceImmediate {
                target: target_register,
                source: source_register,
                immediate,
            },
        );
        assert_eq!(machine.processor.registers[source_register], source_value);
        assert_eq!(machine.processor.registers[target_register], expected_value);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn add_immediate_with_zero_flag_set() {
        let mut machine = Machine::new();
        let target_register = 0xAB.into();
        let source_register = 0x07.into();
        let immediate = 0;
        let source_value = 0;
        let expected_value = 0;
        machine.processor.registers[source_register] = source_value;
        let machine = execute_instruction_with_machine(
            machine,
            AddTargetSourceImmediate {
                target: target_register,
                source: source_register,
                immediate,
            },
        );
        assert_eq!(machine.processor.registers[source_register], source_value);
        assert_eq!(machine.processor.registers[target_register], expected_value);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn add_immediate_with_carry_flag_set() {
        let mut machine = Machine::new();
        let target_register = 0xAB.into();
        let source_register = 0x07.into();
        let immediate = 5;
        let source_value = Word::MAX;
        let expected_value = 4;
        machine.processor.registers[source_register] = source_value;
        let machine = execute_instruction_with_machine(
            machine,
            AddTargetSourceImmediate {
                target: target_register,
                source: source_register,
                immediate,
            },
        );
        assert_eq!(machine.processor.registers[source_register], source_value);
        assert_eq!(machine.processor.registers[target_register], expected_value);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn add_immediate_with_zero_and_carry_flags_set() {
        let mut machine = Machine::new();
        let target_register = 0xAB.into();
        let source_register = 0x07.into();
        let immediate = 1;
        let source_value = Word::MAX;
        let expected_value = 0;
        machine.processor.registers[source_register] = source_value;
        let machine = execute_instruction_with_machine(
            machine,
            AddTargetSourceImmediate {
                target: target_register,
                source: source_register,
                immediate,
            },
        );
        assert_eq!(machine.processor.registers[source_register], source_value);
        assert_eq!(machine.processor.registers[target_register], expected_value);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn subtract_immediate_with_no_flags_set() {
        let mut machine = Machine::new();
        let target_register = 0xAB.into();
        let source_register = 0x07.into();
        let immediate = 2;
        let source_value = 44;
        let expected_value = 42;
        machine.processor.registers[source_register] = source_value;
        let machine = execute_instruction_with_machine(
            machine,
            SubtractTargetSourceImmediate {
                target: target_register,
                source: source_register,
                immediate,
            },
        );
        assert_eq!(machine.processor.registers[source_register], source_value);
        assert_eq!(machine.processor.registers[target_register], expected_value);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn subtract_immediate_with_zero_flag_set() {
        let mut machine = Machine::new();
        let target_register = 0xAB.into();
        let source_register = 0x07.into();
        let immediate = 42;
        let source_value = 42;
        let expected_value = 0;
        machine.processor.registers[source_register] = source_value;
        let machine = execute_instruction_with_machine(
            machine,
            SubtractTargetSourceImmediate {
                target: target_register,
                source: source_register,
                immediate,
            },
        );
        assert_eq!(machine.processor.registers[source_register], source_value);
        assert_eq!(machine.processor.registers[target_register], expected_value);
        assert!(machine.processor.get_flag(Flag::Zero));
        assert!(!machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn subtract_immediate_with_carry_flag_set() {
        let mut machine = Machine::new();
        let target_register = 0xAB.into();
        let source_register = 0x07.into();
        let immediate = 2;
        let source_value = 1;
        let expected_value = Word::MAX;
        machine.processor.registers[source_register] = source_value;
        let machine = execute_instruction_with_machine(
            machine,
            SubtractTargetSourceImmediate {
                target: target_register,
                source: source_register,
                immediate,
            },
        );
        assert_eq!(machine.processor.registers[source_register], source_value);
        assert_eq!(machine.processor.registers[target_register], expected_value);
        assert!(!machine.processor.get_flag(Flag::Zero));
        assert!(machine.processor.get_flag(Flag::Carry));
    }

    #[test]
    fn compare_lower_value_against_higher_value() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = 10;
        let rhs = 12;
        let expected = Word::MAX;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            CompareTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn compare_higher_value_against_lower_value() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = 14;
        let rhs = 12;
        let expected = 1;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            CompareTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(!machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn compare_equal_values() {
        let lhs_register = 0x42.into();
        let rhs_register = 0x43.into();
        let target_register = 0x0A.into();
        let lhs = 12;
        let rhs = 12;
        let expected = 0;
        let mut machine = Machine::new();
        machine.processor.registers[lhs_register] = lhs;
        machine.processor.registers[rhs_register] = rhs;
        let mut machine = execute_instruction_with_machine(
            machine,
            CompareTargetLhsRhs {
                target: target_register,
                lhs: lhs_register,
                rhs: rhs_register,
            },
        );
        machine.make_tick();
        assert_eq!(machine.processor.registers[lhs_register], lhs);
        assert_eq!(machine.processor.registers[rhs_register], rhs);
        assert_eq!(machine.processor.registers[target_register], expected);
        assert!(machine.processor.get_flag(Flag::Zero));
    }

    #[test]
    fn push_and_pop_stack_value() {
        let mut machine = Machine::new();
        let source_register = 0xAB.into();
        let target_register = 0x06.into();
        let data = 42;
        machine.processor.registers[source_register] = data;
        assert_eq!(
            machine.processor.get_stack_pointer(),
            Processor::STACK_START
        );
        let machine = execute_instruction_with_machine(
            machine,
            PushRegister {
                register: source_register,
            },
        );
        assert_eq!(
            machine.processor.get_stack_pointer(),
            Processor::STACK_START + Word::SIZE as Address
        );
        assert_eq!(machine.memory.read_data(Processor::STACK_START), data);
        let machine = execute_instruction_with_machine(
            machine,
            PopRegister {
                register: target_register,
            },
        );
        assert_eq!(
            machine.processor.get_stack_pointer(),
            Processor::STACK_START
        );
        assert_eq!(machine.processor.registers[target_register], data);
    }

    #[test]
    fn push_and_pop_multiple_stack_values() {
        let values = [1, 4, 5, 42, 2, 3];
        let mut machine = Machine::new();
        for (register, value) in (0..).map(Register).zip(values) {
            machine.processor.registers[register] = value;
            machine = execute_instruction_with_machine(machine, PushRegister { register });
            assert_eq!(
                machine.processor.get_stack_pointer(),
                Processor::STACK_START + (register.0 as Address + 1) * Word::SIZE as Address
            );
            assert_eq!(
                machine.memory.read_data(
                    Processor::STACK_START + register.0 as Address * Word::SIZE as Address
                ),
                value
            );
        }
        for &value in values.iter().rev() {
            let target = 0xAB.into();
            machine = execute_instruction_with_machine(machine, PopRegister { register: target });
            assert_eq!(machine.processor.registers[target], value);
        }
        assert_eq!(
            machine.processor.get_stack_pointer(),
            Processor::STACK_START
        );
    }

    #[test]
    fn call_and_return() {
        let mut machine = Machine::new();
        let call_address = Processor::ENTRY_POINT + 200 * Instruction::SIZE as Address;
        machine.memory.write_opcode(
            Processor::ENTRY_POINT,
            Opcode::CallAddress {
                address: call_address,
            },
        );
        let target_register = Register(0xAB);
        let value = 42;
        machine.memory.write_opcode(
            call_address,
            Opcode::MoveRegisterImmediate {
                register: target_register,
                immediate: value,
            },
        );
        machine.memory.write_opcode(
            call_address + Instruction::SIZE as Address,
            Opcode::Return {},
        );

        machine.make_tick(); // jump into subroutine
        assert_eq!(
            machine.memory.read_data(Processor::STACK_START),
            Processor::ENTRY_POINT + Instruction::SIZE as Address
        );
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            call_address
        );

        machine.make_tick(); // write value into register
        assert_eq!(machine.processor.registers[target_register], value);
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            call_address + Instruction::SIZE as Address
        );

        machine.make_tick(); // jump back from subroutine
        assert_eq!(
            machine.processor.registers[Processor::INSTRUCTION_POINTER],
            Processor::ENTRY_POINT + Instruction::SIZE as Address
        );
    }

    create_test!(
        jump_to_address,
        setup = {
            let address = Processor::ENTRY_POINT as Address + 42;
        },
        opcodes = &[Opcode::JumpAddress { address }],
        registers_post = [(Processor::INSTRUCTION_POINTER, address)],
    );

    create_test!(
        jump_to_pointer,
        setup = {
            let register = Register(0xAB);
            let address = Processor::ENTRY_POINT as Address + 42;
        },
        opcodes = &[Opcode::JumpRegister { register }],
        registers_pre = [address => register],
        registers_post = [(Processor::INSTRUCTION_POINTER, address)],
    );

    macro_rules! create_jump_test {
        ($test_name:ident,
        $jump_instruction:ident,
        $lhs:literal,
        $rhs:literal,
        $should_jump:literal) => {
            create_test!(
                $test_name,
                setup = {
                    let target_address = Processor::ENTRY_POINT + 42 * Instruction::SIZE as Address;
                    let target_register = 0.into();
                },
                opcodes = &[
                    Opcode::CompareTargetLhsRhs {
                        target: target_register,
                        lhs: 1.into(),
                        rhs: 2.into(),
                    },
                    Opcode::$jump_instruction {
                        register: target_register,
                        address: target_address,
                    },
                ],
                registers_pre = [$lhs => 1, $rhs => 2],
                registers_post = [(Processor::INSTRUCTION_POINTER, if $should_jump { target_address } else {
                    Processor::ENTRY_POINT + 2 * Instruction::SIZE as Address
                })],
            );
        };
    }

    create_jump_test!(
        jump_to_address_if_equal_that_jumps,
        JumpAddressIfEqual,
        42,
        42,
        true
    );

    create_jump_test!(
        jump_to_address_if_equal_that_does_not_jump,
        JumpAddressIfEqual,
        42,
        43,
        false
    );

    create_jump_test!(
        jump_to_address_if_greater_than_that_jumps,
        JumpAddressIfGreaterThan,
        43,
        42,
        true
    );

    create_jump_test!(
        jump_to_address_if_greater_than_that_does_not_jump_01,
        JumpAddressIfGreaterThan,
        42,
        43,
        false
    );

    create_jump_test!(
        jump_to_address_if_greater_than_that_does_not_jump_02,
        JumpAddressIfGreaterThan,
        42,
        42,
        false
    );

    create_jump_test!(
        jump_to_address_if_less_than_that_jumps,
        JumpAddressIfLessThan,
        41,
        42,
        true
    );

    create_jump_test!(
        jump_to_address_if_less_than_that_does_not_jump_01,
        JumpAddressIfLessThan,
        43,
        42,
        false
    );

    create_jump_test!(
        jump_to_address_if_less_than_that_does_not_jump_02,
        JumpAddressIfLessThan,
        42,
        42,
        false
    );

    create_jump_test!(
        jump_to_address_if_less_than_or_equal_that_jumps_01,
        JumpAddressIfLessThanOrEqual,
        41,
        42,
        true
    );

    create_jump_test!(
        jump_to_address_if_less_than_or_equal_that_jumps_02,
        JumpAddressIfLessThanOrEqual,
        42,
        42,
        true
    );

    create_jump_test!(
        jump_to_address_if_less_than_or_equal_that_does_not_jump,
        JumpAddressIfLessThanOrEqual,
        43,
        42,
        false
    );

    create_jump_test!(
        jump_to_address_if_greater_than_or_equal_that_jumps_01,
        JumpAddressIfGreaterThanOrEqual,
        43,
        42,
        true
    );

    create_jump_test!(
        jump_to_address_if_greater_than_or_equal_that_jumps_02,
        JumpAddressIfGreaterThanOrEqual,
        42,
        42,
        true
    );

    create_jump_test!(
        jump_to_address_if_greater_than_or_equal_that_does_not_jump,
        JumpAddressIfGreaterThanOrEqual,
        41,
        42,
        false
    );

    macro_rules! create_jump_flag_test(
        (
            $test_name:ident,
            $jump_instruction:ident,
            $lhs:expr,
            $rhs:expr,
            $should_jump:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let target_address = Processor::ENTRY_POINT + 42 * Instruction::SIZE as Address;
                    let high_register = 3.into();
                    let target_register = 0.into();
                },
                opcodes = &[
                    Opcode::MultiplyHighLowLhsRhs {
                        high: high_register,
                        low: target_register,
                        lhs: 1.into(),
                        rhs: 2.into(),
                    },
                    Opcode::$jump_instruction {
                        address: target_address,
                    },
                ],
                registers_pre = [$lhs => 1, $rhs => 2],
                registers_post = [(Processor::INSTRUCTION_POINTER, if $should_jump { target_address } else {
                    Processor::ENTRY_POINT + 2 * Instruction::SIZE as Address
                })],
            );
        }
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_set_that_jumps,
        JumpAddressIfZero,
        5,
        0,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_set_that_does_not_jump,
        JumpAddressIfZero,
        5,
        2,
        false
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_not_set_that_jumps,
        JumpAddressIfNotZero,
        5,
        3,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_zero_flag_not_set_that_does_not_jump,
        JumpAddressIfNotZero,
        5,
        0,
        false
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_set_that_jumps,
        JumpAddressIfCarry,
        Word::MAX,
        2,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_set_that_does_not_jump,
        JumpAddressIfCarry,
        5,
        2,
        false
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_not_set_that_jumps,
        JumpAddressIfNotCarry,
        5,
        3,
        true
    );

    create_jump_flag_test!(
        jump_to_address_if_carry_flag_not_set_that_does_not_jump,
        JumpAddressIfNotCarry,
        2,
        Word::MAX,
        false
    );

    macro_rules! create_jump_divmod_test {
        (
            $test_name:ident,
            $jump_instruction:ident,
            $lhs:expr,
            $rhs:expr,
            $should_jump:literal
        ) => {
            create_test!(
                $test_name,
                setup = {
                    let target_address = Processor::ENTRY_POINT + 42 * Instruction::SIZE as Address;
                    let remainder_register = 3.into();
                    let target_register = 0.into();
                },
                opcodes = &[
                    Opcode::DivmodTargetModLhsRhs {
                        result: target_register,
                        remainder: remainder_register,
                        lhs: 1.into(),
                        rhs: 2.into(),
                    },
                    Opcode::$jump_instruction {
                        address: target_address,
                    },
                ],
                registers_pre = [$lhs => 1, $rhs => 2],
                registers_post = [(Processor::INSTRUCTION_POINTER, if $should_jump { target_address } else {
                    Processor::ENTRY_POINT + 2 * Instruction::SIZE as Address
                })],
            );
        };
    }

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_set_that_jumps,
        JumpAddressIfDivideByZero,
        5,
        0,
        true
    );

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_set_that_does_not_jump,
        JumpAddressIfDivideByZero,
        5,
        2,
        false
    );

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_not_set_that_jumps,
        JumpAddressIfNotDivideByZero,
        5,
        3,
        true
    );

    create_jump_divmod_test!(
        jump_to_address_if_divide_by_zero_flag_not_set_that_does_not_jump,
        JumpAddressIfNotDivideByZero,
        2,
        0,
        false
    );
}
