use std::fs::read_to_string;
use std::str::FromStr;

use crate::sdk::doc_input::Table;
use crate::sdk::{BitField, Bits, Peripheral, Register, Type};

struct Row {
    address: Option<u32>,
    reg_name: String,
    signal: String,
    bit_pos: Option<Bits>,
    default: Option<u32>,
    ty: Option<Type>,
    description: String,
}

fn decode_table(input: Table) -> Peripheral {
    let gpio_mode = input.header[0] == "NUM";
    let mut peripheral = Peripheral::default();

    let mut reg = Register::default();
    let mut last_type = Type::ReadWrite;
    for line in input.data {
        if line[0].contains('~') {
            continue;
        }

        let row = extract_row(line, gpio_mode);
        if row.address.is_some() {
            // start of new register, push the old one
            if !reg.name.is_empty() {
                peripheral.registers.push(reg.clone());
            }

            reg = Register::default();
            reg.width = 32;
            reg.address = row.address.unwrap();
            reg.name = row.reg_name.trim_end_matches("_ADDRESS").to_string();
            reg.description = if row.description.is_empty() {
                reg.name.clone()
            } else {
                row.description.clone()
            };
        }

        if !row.signal.is_empty()
            && row.bit_pos.is_some()
            && (row.ty.is_some() || !row.signal.is_empty())
        {
            // start of new bitfield
            let bit_field = BitField {
                name: row.signal,
                bits: row.bit_pos.unwrap(),
                description: row.description,
                reset_value: row.default.unwrap_or_default(),
                type_: row.ty.unwrap_or(last_type),
            };

            last_type = bit_field.type_;
            reg.bit_fields.push(bit_field);
        } else if !row.description.is_empty() {
            if let Some(last_bit_field) = reg.bit_fields.last_mut() {
                last_bit_field.description.push_str(", ");
                last_bit_field.description.push_str(&row.description)
            }
        }
    }

    peripheral
}

fn extract_row(line: Vec<String>, gpio_mode: bool) -> Row {
    if gpio_mode {
        let mut parts = line.into_iter().skip(1).map(|part| part.replace('\r', ""));
        let address = parts.next().unwrap();
        let _ = parts.next();
        let reg_name = parts.next().unwrap();
        let signal = parts.next().unwrap();
        let bit_pos = parts.next().unwrap();
        let sw = parts.next().unwrap();
        let description = parts.next().unwrap();

        Row {
            address: parse_addr(&address).map(|addr| addr * 4),
            reg_name,
            signal,
            bit_pos: parse_bits(&bit_pos),
            default: None,
            ty: Type::from_str(&sw).ok(),
            description,
        }
    } else {
        let mut parts = line.into_iter().map(|part| part.replace('\r', ""));
        let mut address = parts.next().unwrap();
        let reg_name = parts.next().unwrap();
        let signal = parts.next().unwrap();
        let bit_pos = parts.next().unwrap();
        let default = parts.next().unwrap();
        let sw = parts.next().unwrap();
        let description = parts.next().unwrap();

        // broken row in the table
        if reg_name == "UART_STATUS" {
            address = "0x1c".to_string();
        }

        Row {
            address: parse_addr(&address),
            reg_name,
            signal,
            bit_pos: parse_bits(&bit_pos),
            default: parse_default(&default),
            ty: Type::from_str(&sw).ok(),
            description,
        }
    }
}

fn parse_addr(addr: &str) -> Option<u32> {
    if addr.is_empty() || addr.contains('~') {
        return None;
    }
    Some(u32::from_str_radix(&addr.trim_start_matches("0x"), 16).unwrap())
}

fn parse_bits(bit_pos: &str) -> Option<Bits> {
    if bit_pos.is_empty() {
        return None;
    }

    let nums = bit_pos.trim_start_matches('[').trim_end_matches(']');
    let parts: Vec<u8> = nums
        .split(':')
        .map(|digits| u8::from_str(digits).unwrap())
        .collect();

    Some(if parts.len() == 1 {
        Bits::Single(parts[0])
    } else {
        Bits::Range(parts[1]..=parts[0])
    })
}

fn parse_default(default: &str) -> Option<u32> {
    if default.is_empty() {
        return None;
    }

    let default = default.split('\'').skip(1).next().unwrap().replace('_', "");
    Some(match &default[..1] {
        "b" => u32::from_str_radix(&default[1..], 2).unwrap(),
        "d" => u32::from_str_radix(&default[1..], 10).unwrap(),
        "h" => u32::from_str_radix(&default[1..], 16).unwrap(),
        _ => panic!("invalid default format"),
    })
}

pub fn parse_doc(name: &str) -> Peripheral {
    let file = read_to_string(name).unwrap();
    let input = serde_json::from_str(&file).unwrap();

    decode_table(input)
}
