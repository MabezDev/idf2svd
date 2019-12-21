pub const SOC_BASE_PATH: &'static str = "esp-idf/components/soc/esp32/include/soc/";

use header2svd::{parse_idf, Bits, Peripheral};

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::mem;
use std::ptr;
use svd_parser::{
    bitrange::BitRangeType, encode::Encode, BitRange, Field,
    FieldInfo, /* Interrupt as SvdInterrupt ,*/
    Peripheral as SvdPeripheral, Register as SvdRegister, RegisterCluster, RegisterInfo,
    RegisterProperties,
};
use xmltree::Element;

fn main() {
    let peripherals = parse_idf(SOC_BASE_PATH);

    let svd = create_svd(peripherals).unwrap();

    let f = BufWriter::new(File::create("esp32.svd").unwrap());
    svd.write(f).unwrap();
}

fn create_svd(peripherals: HashMap<String, Peripheral>) -> Result<Element, ()> {
    let mut svd_peripherals = vec![];

    for (name, p) in peripherals {
        /*
           Note on unsafe:
           `mem::unitialized` is the only way to create these svd peripherals currently.
           We must ensure that **all** the fields are initialized with a value
           failure to do so will result in undefined behaviour when the value is dropped
        */
        let mut out: SvdPeripheral = unsafe { mem::zeroed() };

        let mut registers = vec![];
        for r in p.registers {
            let mut info: RegisterInfo = unsafe { mem::zeroed() };
            unsafe {
                ptr::write_unaligned(&mut info.name, r.name.clone());
                ptr::write_unaligned(&mut info.alternate_group, None);
                ptr::write_unaligned(&mut info.alternate_register, None);
                ptr::write_unaligned(&mut info.derived_from, None);
                if info.derived_from.is_some() {
                    panic!("how can this be?");
                }
                ptr::write_unaligned(&mut info.description, Some(r.description.clone()));
                ptr::write_unaligned(&mut info.address_offset, r.address);
                ptr::write_unaligned(&mut info.size, Some(32)); // TODO calc width

                // TODO
                ptr::write_unaligned(&mut info.access, None);
                ptr::write_unaligned(&mut info.reset_value, Some(r.reset_value as u32));
                ptr::write_unaligned(&mut info.reset_mask, None);

                let mut fields = vec![];
                for field in &r.bit_fields {
                    let mut field_out: FieldInfo = mem::zeroed();
                    ptr::write_unaligned(&mut field_out.name, field.name.clone());
                    ptr::write_unaligned(
                        &mut field_out.description,
                        if field.description.trim().is_empty() {
                            None
                        } else {
                            Some(field.description.clone())
                        },
                    );
                    ptr::write_unaligned(
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
                    ptr::write_unaligned(&mut field_out.access, Some(field.type_.into()));
                    ptr::write_unaligned(&mut field_out.enumerated_values, vec![]);
                    ptr::write_unaligned(&mut field_out.write_constraint, None);
                    ptr::write_unaligned(&mut field_out.modified_write_values, None);

                    fields.push(Field::Single(field_out));
                }

                ptr::write_unaligned(&mut info.fields, Some(fields));
                ptr::write_unaligned(&mut info.write_constraint, None);
                ptr::write_unaligned(&mut info.modified_write_values, None);
            }

            registers.push(RegisterCluster::Register(SvdRegister::Single(info)));
        }

        unsafe {
            ptr::write_unaligned(&mut out.name, name.to_owned());
            ptr::write_unaligned(&mut out.version, None);
            ptr::write_unaligned(&mut out.display_name, None);
            ptr::write_unaligned(&mut out.group_name, None);
            ptr::write_unaligned(&mut out.description, None);
            ptr::write_unaligned(&mut out.base_address, p.address);
            ptr::write_unaligned(&mut out.address_block, None);
            // TODO parse interrupt information
            ptr::write_unaligned(&mut out.interrupt, vec![]);

            // TODO parse this information properly
            let mut drp: RegisterProperties = mem::zeroed();
            ptr::write_unaligned(&mut drp.access, None);
            ptr::write_unaligned(&mut drp.reset_mask, None);
            ptr::write_unaligned(&mut drp.reset_value, None);
            ptr::write_unaligned(&mut drp.size, None);

            ptr::write_unaligned(&mut out.default_register_properties, drp);
            ptr::write_unaligned(&mut out.registers, Some(registers));
            ptr::write_unaligned(&mut out.derived_from, None); // first.as_ref().map(|s| s.to_owned())
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
    Ok(out)
}
