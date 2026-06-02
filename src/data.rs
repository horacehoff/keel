use crate::{string_gc::raise_string_gc_threshold, string_gc::string_gc, vm::ObjectPool};

// 51 bits of total payload => 3 bits for data type & 48 bits of actual payload
// 1111_1111_1111_1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000
// |    NAN TAG   |                    TYPE TAG + PAYLOAD                        |
const NAN_BASE: u64 =
    0b1111_1111_1111_1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
const PAYLOAD_MASK: u64 = 0b1111_1111_1111_1111_1111_1111_1111_1111_1111_1111_1111_1111;
const NAN_BOOL: u64 = NAN_BASE | (1 << 48);
const NAN_STRING_SMALL: u64 = NAN_BASE | (2 << 48);
const NAN_STRING_LARGE: u64 = NAN_BASE | (3 << 48);
const NAN_ARRAY: u64 = NAN_BASE | (4 << 48);
const NAN_NULL: u64 = NAN_BASE | (5 << 48);
const NAN_INT: u64 = NAN_BASE | (6 << 48);
const NAN_STRUCT: u64 = NAN_BASE | (7 << 48);
const BOOL_TABLE: [Data; 2] = [FALSE, TRUE];
pub const NULL: Data = Data(NAN_NULL);
pub const FALSE: Data = Data(NAN_BOOL);
pub const TRUE: Data = Data(NAN_BOOL | 1);

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Data(pub u64);

impl Data {
    #[inline(always)]
    pub fn tag(&self) -> u64 {
        self.0 & !0xFFFF_FFFF // also includes type_id
    }
    #[inline(always)]
    pub fn is_null(&self) -> bool {
        self.0 == NAN_NULL
    }
    #[inline(always)]
    pub fn bool(b: bool) -> Data {
        Data(NAN_BOOL | b as u64)
    }
    #[inline(always)]
    pub fn as_bool(&self) -> bool {
        debug_assert!(self.is_bool());
        (self.0 & 1) != 0
    }
    #[inline(always)]
    pub fn is_bool(&self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_BOOL
    }
    #[inline(always)]
    pub fn float(n: f64) -> Data {
        Data(n.to_bits())
    }
    #[inline(always)]
    pub fn as_float(&self) -> f64 {
        debug_assert!(self.is_float());
        f64::from_bits(self.0)
    }
    #[inline(always)]
    pub fn is_float(&self) -> bool {
        (self.0 & NAN_BASE) != NAN_BASE
    }
    #[inline(always)]
    /// Convert the given integer to a NaN-boxed integer.
    /// Integers are stored in the lower 32 bits
    pub fn int(n: i32) -> Data {
        Data(NAN_INT | (n as u32 as u64))
    }
    #[inline(always)]
    pub fn as_int(&self) -> i32 {
        debug_assert!(self.is_int());
        self.0 as i32
    }
    #[inline(always)]
    pub fn is_int(&self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_INT
    }
    #[inline(always)]
    pub fn array(id: u32) -> Data {
        Data(NAN_ARRAY | id as u64)
    }
    #[inline(always)]
    pub fn as_array(&self) -> usize {
        debug_assert!(self.is_array() || self.is_struct());
        (self.0 & 0xFFFF_FFFF) as usize
    }
    #[inline(always)]
    pub fn is_array(&self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_ARRAY
    }
    #[inline(always)]
    pub fn small_str(s: &str) -> Data {
        debug_assert!(s.len() <= 6);
        let bytes = s.as_bytes();
        let mut payload: u64 = 0;
        // Packs 6 bytes into the payload, filling up the 48 payload bits
        for (i, byte) in bytes.iter().enumerate() {
            payload |= (*byte as u64) << (i * 8);
        }
        Data(NAN_STRING_SMALL | (payload & PAYLOAD_MASK))
    }
    #[inline(always)]
    pub fn large_str_id(id: u64) -> Data {
        Data(NAN_STRING_LARGE | id)
    }
    #[inline(always)]
    /// Same as str(), except this never runs the GC because this function is called by the parser
    pub fn p_str(s: &str, string_pool: &mut Vec<String>) -> Data {
        let len = s.len();
        if len <= 6 {
            Data::small_str(s)
        } else {
            let string_pool_id = string_pool.len() as u64;
            string_pool.push(s.to_string());
            Data(NAN_STRING_LARGE | string_pool_id)
        }
    }
    #[inline(always)]
    /// Allocates a string, storing it directly inside the u64 if it's <= 6 characters or inside string_pool if it's bigger
    pub fn str(
        s: &str,
        array_pool: &ObjectPool,
        string_pool: &mut Vec<String>,
        registers: &[Data],
        recursion_stack: &[Data],
        free_strings: &mut Vec<u16>,
        gc_string_threshold: &mut u32,
        string_live: &mut Vec<bool>,
    ) -> Data {
        if s.len() <= 6 {
            Data::small_str(s)
        } else {
            if free_strings.is_empty() && string_pool.len() >= (*gc_string_threshold as usize) {
                raise_string_gc_threshold(gc_string_threshold, string_pool.len());
                string_gc(
                    array_pool,
                    string_pool,
                    free_strings,
                    registers,
                    recursion_stack,
                    string_live,
                );
            }
            if let Some(id) = free_strings.pop() {
                string_pool[id as usize] = s.to_string();
                Data(NAN_STRING_LARGE | (id as u64))
            } else {
                let string_pool_id = string_pool.len() as u64;
                string_pool.push(s.to_string());
                Data(NAN_STRING_LARGE | string_pool_id)
            }
        }
    }
    #[inline(always)]
    pub fn string(
        s: String,
        array_pool: &ObjectPool,
        string_pool: &mut Vec<String>,
        registers: &[Data],
        recursion_stack: &[Data],
        free_strings: &mut Vec<u16>,
        gc_string_threshold: &mut u32,
        string_live: &mut Vec<bool>,
    ) -> Data {
        if s.len() <= 6 {
            Data::small_str(&s)
        } else {
            if free_strings.is_empty() && string_pool.len() >= (*gc_string_threshold as usize) {
                raise_string_gc_threshold(gc_string_threshold, string_pool.len());
                string_gc(
                    array_pool,
                    string_pool,
                    free_strings,
                    registers,
                    recursion_stack,
                    string_live,
                );
            }
            if let Some(id) = free_strings.pop() {
                string_pool[id as usize] = s;
                Data(NAN_STRING_LARGE | (id as u64))
            } else {
                let string_pool_id = string_pool.len() as u64;
                string_pool.push(s);
                Data(NAN_STRING_LARGE | string_pool_id)
            }
        }
    }
    #[inline(always)]
    pub fn as_str(&self, string_pool: &[String]) -> &str {
        debug_assert!(self.is_str());
        if (self.0 & !PAYLOAD_MASK) == NAN_STRING_SMALL {
            let payload = self.0 & PAYLOAD_MASK;
            let len = ((64 - payload.leading_zeros()) as usize + 7) >> 3;
            let ptr = self as *const Data as *const u8;
            unsafe {
                let slice = std::slice::from_raw_parts(ptr, len);
                std::str::from_utf8_unchecked(slice)
            }
        } else {
            let payload = (self.0 & PAYLOAD_MASK) as usize;
            unsafe { &*(string_pool.get_unchecked(payload).as_str() as *const str) }
        }
    }
    #[inline(always)]
    pub fn is_str(&self) -> bool {
        // this works because NAN_TAG_STRING_LARGE == NAN_TAG_STRING_SMALL + (1 << 48)
        (self.0 & !PAYLOAD_MASK).wrapping_sub(NAN_STRING_SMALL) <= const { 1u64 << 48 }
    }
    /// Increments the integer stored in this Data in-place. Wraps.
    #[inline(always)]
    pub fn inc_int(&mut self) {
        debug_assert!(self.is_int());
        self.0 = NAN_INT | (self.0.wrapping_add(1) & 0xFFFF_FFFF);
    }
    /// Decrements the integer stored in this Data in-place. Wraps.
    #[inline(always)]
    pub fn dec_int(&mut self) {
        debug_assert!(self.is_int());
        self.0 = NAN_INT | (self.0.wrapping_sub(1) & 0xFFFF_FFFF);
    }
    /// Writes src + 1 into self. Wraps.
    #[inline(always)]
    pub fn inc_into(&mut self, src: Data) {
        debug_assert!(src.is_int());
        self.0 = NAN_INT | (src.0.wrapping_add(1) & 0xFFFF_FFFF);
    }
    /// Writes src - 1 into self. Wraps.
    #[inline(always)]
    pub fn dec_into(&mut self, src: Data) {
        debug_assert!(src.is_int());
        self.0 = NAN_INT | (src.0.wrapping_sub(1) & 0xFFFF_FFFF);
    }
    #[inline(always)]
    pub fn is_large_str(&self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_STRING_LARGE
    }
    #[inline(always)]
    pub fn get_str_pool_id(&self) -> usize {
        debug_assert!(self.is_large_str());
        (self.0 & PAYLOAD_MASK) as usize
    }
    #[inline(always)]
    pub fn struct_instance(type_id: u16, id: u32) -> Data {
        Data(NAN_STRUCT | ((type_id as u64) << 32) | id as u64)
    }
    #[inline(always)]
    pub fn as_struct(&self) -> usize {
        debug_assert!(self.is_struct());
        (self.0 & 0xFFFF_FFFF) as usize
    }
    #[inline(always)]
    pub fn struct_type_id(&self) -> u16 {
        ((self.0 >> 32) & 0xFFFF) as u16
    }
    #[inline(always)]
    pub fn is_struct(&self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_STRUCT
    }
}

impl From<f64> for Data {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Data::float(value)
    }
}
impl From<Data> for f64 {
    #[inline(always)]
    fn from(value: Data) -> Self {
        value.as_float()
    }
}

impl From<i32> for Data {
    #[inline(always)]
    fn from(value: i32) -> Self {
        Data::int(value)
    }
}
impl From<Data> for i32 {
    #[inline(always)]
    fn from(value: Data) -> Self {
        value.as_int()
    }
}

impl From<bool> for Data {
    #[inline(always)]
    fn from(value: bool) -> Self {
        BOOL_TABLE[value as usize]
    }
}
impl From<Data> for bool {
    #[inline(always)]
    fn from(value: Data) -> Self {
        value.as_bool()
    }
}
// impl From<&str> for Data {
//     #[inline(always)]
//     fn from(value: &str) -> Self {
//         Data::str(value)
//     }
// }
// impl From<String> for Data {
//     #[inline(always)]
//     fn from(value: String) -> Self {
//         Data::str(&value)
//     }
// }
// impl From<SmolStr> for Data {
//     #[inline(always)]
//     fn from(value: SmolStr) -> Self {
//         Data::str(&value)
//     }
// }
