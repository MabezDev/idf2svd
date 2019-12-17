/* Regex to find all the peripheral base addresses */
pub const REG_BASE: &'static str = r"\#define[\s*]+DR_REG_(.*)_BASE[\s*]+0x([0-9a-fA-F]+)";

/*  */
pub const REG_DEF: &'static str = r"\#define[\s*]+([^\s*]+)[\s*]+\(DR_REG_(.*)_BASE \+ (.*)\)";
pub const REG_DEF_INDEX: &'static str =
    r"\#define[\s*]+([^\s*]+)[\s*]+\(([0-9A-Za-z_]+_BASE)[\s*]*\(i\) \+ (.*)\)";
pub const REG_BITS: &'static str =
    r"#define[\s*]+([^\s*]+)_(S|V)[\s*]+\(?(0x[0-9a-fA-F]+|[0-9]+)\)?";

pub struct Peripheral {
    name: String,
    address: u32,
}
