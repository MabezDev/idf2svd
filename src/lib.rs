
use std::ops::RangeInclusive;


/* Regex's to find all the peripheral addresses */
pub const REG_BASE: &'static str = r"\#define[\s*]+DR_REG_(.*)_BASE[\s*]+0x([0-9a-fA-F]+)";
pub const REG_DEF: &'static str = r"\#define[\s*]+([^\s*]+)[\s*]+\(DR_REG_(.*)_BASE \+ (.*)\)";
pub const REG_DEF_INDEX: &'static str =
    r"\#define[\s*]+([^\s*]+)[\s*]+\(REG_([0-9A-Za-z_]+)_BASE[\s*]*\(i\) \+ (.*)\)";
pub const REG_BITS: &'static str =
    r"\#define[\s*]+([^\s*]+)_(S|V)[\s*]+\(?(0x[0-9a-fA-F]+|[0-9]+)\)?";
pub const REG_BIT_INFO: &'static str = r":[\s]+([0-9A-Za-z_\/]+)[\s]+;bitpos:\[([0-9]+):?([0-9]+)?\][\s];default:[\s]+(.*)[\s];[\s]\*\/";
pub const REG_DESC: &'static str = r"\*description:\s(.*[\n|\r|\r\n]?.*)\*/"; 
#[derive(Debug, Default)]
pub struct Peripheral {
    pub description: String,
    pub address: u32,
    pub registers: Vec<Register>,
}

#[derive(Debug, Default)]
pub struct Register {
    /// Register Name
    pub name: String,
    /// Relative Address
    pub address: u32,
    /// Width
    pub width: u8,
    /// Description
    pub description: String,
    /// Reset Value
    pub reset_value: u64,
    /// Detailed description
    pub detailed_description: Option<String>,
    pub bit_fields: Vec<BitField>,
}

#[derive(Debug, Default)]
pub struct BitField {
    /// Field Name
    pub name: String,
    /// Bits
    pub bits: Bits,
    /// Type
    // pub type_: Type,
    /// Reset Value
    pub reset_value: u32,
    /// Description
    pub description: String,
}

#[derive(Debug)]
pub enum Bits {
    Single(u8),
    Range(RangeInclusive<u8>),
}

impl Default for Bits {
    fn default() -> Self {
        Bits::Single(0)
    }
}

// #[derive(Debug)]
// pub enum Type {
//     ReadAsZero,
//     ReadOnly,
//     ReadWrite,
//     ReadWriteSetOnly,
//     ReadableClearOnRead,
//     ReadableClearOnWrite,
//     WriteAsZero,
//     WriteOnly,
//     WriteToClear,
// }