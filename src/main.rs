pub const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

use header2svd::{
    BitField, Bits, Peripheral, Register, REG_BASE, REG_BITS, REG_BIT_INFO, REG_DEF, REG_DEF_INDEX,
    REG_DESC,
};
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::mem;
use std::ptr;
use svd_parser::{
    bitrange::BitRangeType, encode::Encode, BitRange, Field, FieldInfo,
    Peripheral as SvdPeripheral, Register as SvdRegister, RegisterCluster, RegisterInfo,
};
use xmltree::Element;

enum State {
    FindReg,
    FindBitFieldInfo(String, Register),
    FindDescription(String, Register, BitField),
    CheckEnd(String, Register),
}

fn main() {
    let mut peripherals = HashMap::new();

    let filname = SOC_BASE_PATH.to_owned() + "soc.h";
    let re_base = Regex::new(REG_BASE).unwrap();
    let re_reg = Regex::new(REG_DEF).unwrap();
    let re_reg_index = Regex::new(REG_DEF_INDEX).unwrap();
    let re_reg_desc = Regex::new(REG_DESC).unwrap();
    let re_reg_bit_info = Regex::new(REG_BIT_INFO).unwrap();
    let re_reg_bits = Regex::new(REG_BITS).unwrap();

    for captures in re_base.captures_iter(file_to_string(&filname).as_str()) {
        let peripheral = &captures[1];
        let address = &captures[2];
        let mut p = Peripheral::default();
        p.address = u32::from_str_radix(address, 16).unwrap();
        p.description = peripheral.to_string();

        peripherals.insert(peripheral.to_string(), p);
    }

    std::fs::read_dir(SOC_BASE_PATH)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| f.path().to_str().unwrap().ends_with("_reg.h"))
        .for_each(|f| {
            let name = f.path();
            let name = name.to_str().unwrap();
            let mut buffer = vec![];
            let file_data = file_to_string(name);
            println!("Searching {}", name);

            let mut state = State::FindReg;
            for line in file_data.lines() {
                loop {
                    match state {
                        State::FindReg => {
                            /* Normal register definitions */
                            if let Some(m) = re_reg.captures(line) {
                                let reg_name = &m[1];
                                let pname = &m[2];
                                let offset = &m[3].trim_start_matches("0x");
                                if reg_name.ends_with("(i)") {
                                    // some indexed still get through, ignore them
                                    break;
                                }
                                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                    let mut r = Register::default();
                                    r.description = reg_name.to_string();
                                    r.name = reg_name.to_string();
                                    r.address = addr;
                                    state = State::FindBitFieldInfo(pname.to_string(), r);
                                } else {
                                    println!(
                                        "Failed to parse register for {}: {}",
                                        reg_name, offset
                                    )
                                }
                            } else if let Some(m) = re_reg_index.captures(line) {
                                let reg_name = &m[1];
                                let pname = &m[2];
                                let offset = &m[3].trim_start_matches("0x");

                                if let Ok(addr) = u32::from_str_radix(offset, 16) {
                                    let mut r = Register::default();
                                    r.description = reg_name.to_string();
                                    r.name = reg_name.to_string();
                                    r.address = addr;
                                    state = State::FindBitFieldInfo(pname.to_string(), r);
                                } else {
                                    println!(
                                        "Failed to parse register for {}: {}",
                                        reg_name, offset
                                    )
                                }
                            }
                            break; // next line
                        }
                        State::FindBitFieldInfo(ref mut pname, ref mut reg) => {
                            if let Some(m) = re_reg_bit_info.captures(line) {
                                let bf_name = &m[1];
                                let _access_type = &m[2]; // TODO
                                let bits = &mut m[3].split(':');
                                let _default_val = &m[4]; // TODO
                                let bits = match (bits.next(), bits.next()) {
                                    (Some(h), Some(l)) => {
                                        Bits::Range(l.parse().unwrap()..=h.parse().unwrap())
                                    }
                                    (Some(b), None) => Bits::Single(b.parse().unwrap()),
                                    _ => {
                                        println!("Failed to parse bitpos {}", &m[3]);
                                        continue;
                                    }
                                };

                                let bf = BitField {
                                    name: bf_name.to_string(),
                                    bits,
                                    reset_value: 0,
                                    ..Default::default()
                                };
                                state = State::FindDescription(pname.clone(), reg.clone(), bf);
                            } else {
                                println!("Failed to match reg info");
                                state = State::FindReg;
                            }
                            break; // next line
                        }
                        State::FindDescription(ref mut pname, ref mut reg, ref mut bf) => {
                            buffer.push(line);
                            if let Some(m) = re_reg_desc.captures(buffer.join("").as_str()) {
                                bf.description = m[1].to_string();
                                buffer.clear();
                                reg.bit_fields.push(bf.clone()); // add the bit field to the reg
                                state = State::CheckEnd(pname.clone(), reg.clone());
                            }
                            break; // next line
                        }
                        State::CheckEnd(ref mut pname, ref mut reg) => {
                            if line.is_empty() {
                                // println!("{} Adding {:#?}", pname, reg);
                                // were done with this register
                                if let Some(p) = peripherals.get_mut(&pname.to_string()) {
                                    p.registers.push(reg.clone());
                                } else {
                                    // TODO indexed peripherals wont come up here
                                    println!("No periphal called {}", pname.to_string());
                                }
                                state = State::FindReg;
                                break; // next line
                            } else if re_reg_bit_info.is_match(line) {
                                // weve found the next bit field in the reg
                                state = State::FindBitFieldInfo(pname.clone(), reg.clone());
                            } else {
                                break; // next line
                            }
                        }
                    }
                }
            }
        });

    // println!("{:#?}", peripherals);
    create_svd(peripherals).unwrap();
}

fn create_svd(peripherals: HashMap<String, Peripheral>) -> Result<(), ()> {
    let mut svd_peripherals = vec![];

    for (name, p) in peripherals {
        let mut out: SvdPeripheral = unsafe { mem::uninitialized() };

        let mut registers = vec![];
        for r in p.registers {
            let mut info: RegisterInfo = unsafe { mem::uninitialized() };

            unsafe {
                ptr::write(&mut info.name, r.name.clone());
                ptr::write(&mut info.alternate_group, None);
                ptr::write(&mut info.alternate_register, None);
                ptr::write(&mut info.derived_from, None);
                ptr::write(&mut info.description, Some(r.description.clone()));
                ptr::write(&mut info.address_offset, r.address);
                ptr::write(&mut info.size, Some(32)); // TODO calc width

                // TODO
                ptr::write(&mut info.access, None);
                ptr::write(&mut info.reset_value, Some(r.reset_value as u32));
                ptr::write(&mut info.reset_mask, None);

                let mut fields = vec![];
                for field in &r.bit_fields {
                    let mut field_out: FieldInfo = mem::uninitialized();
                    ptr::write(&mut field_out.name, field.name.clone());
                    ptr::write(
                        &mut field_out.description,
                        if field.description.trim().is_empty() {
                            None
                        } else {
                            Some(field.description.clone())
                        },
                    );
                    ptr::write(
                        &mut field_out.bit_range,
                        match &field.bits {
                            Bits::Single(bit) => BitRange {
                                offset: u32::from(*bit),
                                width: 1,
                                range_type: BitRangeType::OffsetWidth,
                            },
                            Bits::Range(r) => BitRange {
                                offset: u32::from(*r.start()),
                                width: u32::from(r.end() - r.start() + 1),
                                range_type: BitRangeType::OffsetWidth,
                            },
                        },
                    );
                    // TODO
                    ptr::write(&mut field_out.access, None);
                    ptr::write(&mut field_out.enumerated_values, vec![]);
                    ptr::write(&mut field_out.write_constraint, None);
                    ptr::write(&mut field_out.modified_write_values, None);

                    fields.push(Field::Single(field_out));
                }

                ptr::write(&mut info.fields, Some(fields));
                ptr::write(&mut info.write_constraint, None);
                ptr::write(&mut info.modified_write_values, None);
            }

            registers.push(RegisterCluster::Register(SvdRegister::Single(info)));
        }

        unsafe {
            ptr::write(&mut out.name, name.to_owned());
            ptr::write(&mut out.version, None);
            ptr::write(&mut out.display_name, None);
            ptr::write(&mut out.group_name, None);
            ptr::write(&mut out.description, None);
            ptr::write(&mut out.base_address, p.address);
            ptr::write(&mut out.address_block, None);
            ptr::write(&mut out.interrupt, vec![]);
            ptr::write(&mut out.registers, Some(registers));
            ptr::write(&mut out.derived_from, None); // first.as_ref().map(|s| s.to_owned())
        }

        svd_peripherals.push(out.encode().unwrap());
    }
    println!("Len {}", svd_peripherals.len());

    let mut children = vec![];
    children.push(Element {
        text: Some("Espressif".to_owned()),
        ..Element::new("name")
    });
    children.push(Element {
        children: svd_peripherals,
        ..Element::new("peripherals")
    });
    let out = Element {
        children: children,
        ..Element::new("device")
    };
    let f = File::create("esp32.svd").unwrap();
    out.write(f).unwrap();
    Ok(())
}

fn file_to_string(fil: &str) -> String {
    let mut soc = File::open(fil).unwrap();
    let mut data = String::new();
    soc.read_to_string(&mut data).unwrap();
    data
}
