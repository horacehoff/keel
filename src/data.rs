use crate::compiler::program::Struct;
use crate::compiler::types::DataType;
use crate::vm::ObjectPool;
use crate::vm::gc::Gc;
use crate::vm::{MapPool, StringPool};
use lexical_core::FormattedSize;
use std::borrow::Cow;

const NAN_BASE: u64 =
    0b1111_1111_1111_1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const PAYLOAD_MASK: u64 = 0b1111_1111_1111_1111_1111_1111_1111_1111_1111_1111_1111_1111;
const NAN_BOOL: u64 = NAN_BASE | (1 << 48);
const NAN_NULL: u64 = NAN_BASE | (2 << 48);
const NAN_INT: u64 = NAN_BASE | (3 << 48);
const NAN_STRING_SMALL: u64 = NAN_BASE | (4 << 48);
const NAN_STRING_LARGE: u64 = NAN_BASE | (5 << 48);
const NAN_ARRAY: u64 = NAN_BASE | (6 << 48);
const NAN_STRUCT: u64 = NAN_BASE | (7 << 48);
const NAN_MAP: u64 = NAN_STRUCT | (1 << 47);
const NAN_FUNCTION: u64 = NAN_MAP | (1 << 46);
pub const NULL: Data = Data(NAN_NULL);
pub const FALSE: Data = Data(NAN_BOOL);
pub const TRUE: Data = Data(NAN_BOOL | 1);

const MATCH_TAG_BOOL: u64 = NAN_BOOL >> 48;
const MATCH_TAG_NULL: u64 = NAN_NULL >> 48;
const MATCH_TAG_INT: u64 = NAN_INT >> 48;
const MATCH_TAG_STRING_SMALL: u64 = NAN_STRING_SMALL >> 48;
const MATCH_TAG_STRING_LARGE: u64 = NAN_STRING_LARGE >> 48;
const MATCH_TAG_ARRAY: u64 = NAN_ARRAY >> 48;
const MATCH_TAG_STRUCT_MAP_FUNCTION: u64 = NAN_STRUCT >> 48;

/// NaN-boxed u64.
/// ### Layout:
/// - first 13 bits are NaN
/// - remaining 51 bits: 3 bits for type (4 for the MAP type) & 48 bits of actual payload (47 for the MAP type)
///
/// ### Data type:
/// - BOOL = 001
/// - NULL = 010
/// - INT = 011
/// - STRING_SMALL = 100
/// - STRING_LARGE = 101
/// - ARRAY = 110
/// - STRUCT = 111
/// - MAP = 1111
///
/// ### Strings
/// Strings are inlined if they're 6 bytes long or less.
/// If they're longer, they're stored in `string_pool`.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Data(pub u64);

impl Data {
    #[inline(always)]
    pub const fn tag(self) -> u64 {
        self.0 & !0xFFFF_FFFF // also includes type_id
    }
    #[inline(always)]
    pub const fn is_null(self) -> bool {
        self.0 == NAN_NULL
    }
    #[inline(always)]
    pub const fn bool(b: bool) -> Self {
        Self(NAN_BOOL | b as u64)
    }
    #[inline(always)]
    pub const fn as_bool(self) -> bool {
        debug_assert!(self.is_bool());
        (self.0 & 1) != 0
    }
    #[inline(always)]
    pub const fn is_bool(self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_BOOL
    }
    #[inline(always)]
    pub const fn float(n: f64) -> Self {
        Self(n.to_bits())
    }
    #[inline(always)]
    pub const fn as_float(self) -> f64 {
        debug_assert!(self.is_float());
        f64::from_bits(self.0)
    }
    #[inline(always)]
    pub const fn is_float(self) -> bool {
        self.0 < NAN_BOOL
    }
    #[inline(always)]
    /// Convert the given integer to a NaN-boxed integer.
    /// Integers are stored in the lower 32 bits
    pub const fn int(n: i32) -> Self {
        Self(NAN_INT | (n as u32 as u64))
    }
    #[inline(always)]
    pub const fn as_int(self) -> i32 {
        debug_assert!(self.is_int());
        self.0 as i32
    }
    #[inline(always)]
    pub const fn is_int(self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_INT
    }
    #[inline(always)]
    pub const fn array(id: u32) -> Self {
        Self(NAN_ARRAY | id as u64)
    }
    #[inline(always)]
    pub const fn as_array(self) -> usize {
        debug_assert!(self.is_array() || self.is_struct());
        (self.0 & 0xFFFF_FFFF) as usize
    }
    #[inline(always)]
    pub const fn is_array(self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_ARRAY
    }
    /// This will create a new inlined string.
    /// The caller guarantees that `s` is never longer than 6 bytes.
    /// In debug it'll panic if it's too big (just in case).
    #[inline(always)]
    pub const fn small_str(s: &str) -> Self {
        debug_assert!(s.len() <= 6);
        let b: &[u8] = s.as_bytes();
        let len = b.len();
        // This avoids a memcpy
        let payload = if len >= 4 {
            (u32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as u64)
                | ((u32::from_ne_bytes([b[len - 4], b[len - 3], b[len - 2], b[len - 1]]) as u64)
                    << ((len - 4) * 8))
        } else if len >= 2 {
            (u16::from_ne_bytes([b[0], b[1]]) as u64)
                | ((u16::from_ne_bytes([b[len - 2], b[len - 1]]) as u64) << ((len - 2) * 8))
        } else if len == 1 {
            b[0] as u64
        } else {
            0
        };
        Self(NAN_STRING_SMALL | payload)
    }
    #[inline(always)]
    pub const fn large_str_id(id: u64) -> Self {
        Self(NAN_STRING_LARGE | id)
    }
    /// Same as str(), except this never runs the GC because this function is called by the compiler
    #[inline(always)]
    pub fn comp_str(s: &str, str_pool: &mut StringPool) -> Self {
        if s.len() <= 6 {
            Self::small_str(s)
        } else if let Some(id) = str_pool.iter().position(|existing| existing == s) {
            Self::large_str_id(id as u64)
        } else {
            let string_pool_id = str_pool.len() as u64;
            str_pool.push(s.to_owned());
            Self::large_str_id(string_pool_id)
        }
    }
    /// Allocates a string, storing it directly inside the u64 if it's <= 6 characters or inside string_pool if it's bigger
    #[inline(always)]
    pub fn string(
        s: Cow<str>,
        obj_pool: &ObjectPool,
        map_pool: &MapPool,
        str_pool: &mut StringPool,
        roots: &[Self],
        gc: &mut Gc,
    ) -> Self {
        if s.len() <= 6 {
            Self::small_str(&s)
        } else {
            let id = gc.alloc_string_slot(obj_pool, map_pool, str_pool, roots);
            let slot = unsafe { str_pool.get_unchecked_mut(id as usize) };
            match s {
                Cow::Borrowed(s) => s.clone_into(slot),
                Cow::Owned(s) => *slot = s,
            }
            Self::large_str_id(u64::from(id))
        }
    }
    #[inline(always)]
    pub fn as_str(&self, str_pool: &StringPool) -> &str {
        debug_assert!(self.is_string());
        let payload = self.0 & PAYLOAD_MASK;
        if (self.0 & !PAYLOAD_MASK) == NAN_STRING_SMALL {
            let len = 8 - (payload.leading_zeros() as usize >> 3);
            let ptr = core::ptr::from_ref::<Self>(self).cast::<u8>();
            unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr, len)) }
        } else {
            unsafe { &*core::ptr::from_ref::<str>(str_pool[payload as usize].as_str()) }
        }
    }
    #[inline(always)]
    pub fn string_eq(self, candidate: Self, str_pool: &StringPool) -> bool {
        self.is_large_str()
            && str_pool[self.get_str_pool_id()] == str_pool[candidate.get_str_pool_id()]
    }
    #[inline(always)]
    pub const fn is_string(self) -> bool {
        // this works because NAN_TAG_STRING_LARGE == NAN_TAG_STRING_SMALL + (1 << 48)
        (self.0 >> 49) == const { NAN_STRING_SMALL >> 49 }
    }
    /// Returns `self + imm`, wraps.
    #[inline(always)]
    #[must_use]
    pub const fn add_int_imm(self, imm: i16) -> Self {
        debug_assert!(self.is_int());
        Self::int(self.as_int().wrapping_add(imm as i32))
    }
    #[inline(always)]
    pub const fn is_large_str(self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_STRING_LARGE
    }
    #[inline(always)]
    pub const fn get_str_pool_id(self) -> usize {
        debug_assert!(self.is_large_str());
        (self.0 & PAYLOAD_MASK) as usize
    }
    #[inline(always)]
    pub const fn struct_instance(type_id: u16, id: u32) -> Self {
        Self(NAN_STRUCT | ((type_id as u64) << 32) | id as u64)
    }
    #[inline(always)]
    pub const fn as_struct(self) -> usize {
        debug_assert!(self.is_struct());
        (self.0 & 0xFFFF_FFFF) as usize
    }
    #[inline(always)]
    pub const fn struct_type_id(self) -> u16 {
        ((self.0 >> 32) & 0xFFFF) as u16
    }
    #[inline(always)]
    pub const fn is_struct(self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_STRUCT && (self.0 & (1 << 47)) == 0
    }
    #[inline(always)]
    pub const fn map(id: u32) -> Self {
        Self(NAN_MAP | id as u64)
    }
    #[inline(always)]
    pub const fn as_map(self) -> usize {
        debug_assert!(self.is_map());
        (self.0 & 0xFFFF_FFFF) as usize
    }
    #[inline(always)]
    pub const fn is_map(self) -> bool {
        (self.0 & !PAYLOAD_MASK) == NAN_STRUCT
            && (self.0 & (1 << 47)) != 0
            && (self.0 & (1 << 46)) == 0
    }
    #[inline(always)]
    pub const fn function(fn_instruction_address: u32) -> Self {
        Self(NAN_FUNCTION | fn_instruction_address as u64)
    }
    /// True for STRING_LARGE, ARRAY, STRUCT, MAP, and FUNCTION.
    #[inline(always)]
    pub const fn is_heap(self) -> bool {
        (self.0 & !PAYLOAD_MASK) >= NAN_STRING_LARGE
    }
    #[inline(always)]
    pub const fn is_function(self) -> bool {
        self.0 >= NAN_FUNCTION
    }
    #[inline(always)]
    #[must_use]
    pub const fn return_point(return_addr: u32, frame_offset: u16) -> Self {
        Self(NAN_INT | ((frame_offset as u64) << 32) | return_addr as u64)
    }
    #[inline(always)]
    #[must_use]
    pub const fn as_return_point(self) -> (u32, u16) {
        (self.0 as u32, (self.0 >> 32) as u16)
    }
    pub fn format(
        self,
        obj_pool: &ObjectPool,
        str_pool: &StringPool,
        map_pool: &MapPool,
        structs: &[Struct],
        show_str_quotation_marks: bool,
    ) -> String {
        let mut output = String::with_capacity(4);
        self.format_to(
            &mut output,
            obj_pool,
            str_pool,
            map_pool,
            structs,
            show_str_quotation_marks,
        );
        output
    }
    #[inline(always)]
    pub fn format_scalar_to(
        self,
        output: &mut String,
        str_pool: &StringPool,
        append_newline: bool,
    ) -> bool {
        let mut buffer = [0u8; i32::FORMATTED_SIZE_DECIMAL];
        let mut float_buffer = zmij::Buffer::new();
        let text = match self.0 >> 48 {
            MATCH_TAG_INT => unsafe {
                str::from_utf8_unchecked(lexical_core::write(self.as_int(), &mut buffer))
            },
            MATCH_TAG_BOOL => {
                if self.as_bool() {
                    "true"
                } else {
                    "false"
                }
            }
            MATCH_TAG_STRING_SMALL | MATCH_TAG_STRING_LARGE => self.as_str(str_pool),
            MATCH_TAG_NULL => "null",
            MATCH_TAG_STRUCT_MAP_FUNCTION if self.is_function() => "function",
            MATCH_TAG_ARRAY | MATCH_TAG_STRUCT_MAP_FUNCTION => return false,
            _ => {
                debug_assert!(self.is_float());
                float_buffer.format(self.as_float())
            }
        };
        unsafe {
            let bytes = output.as_mut_vec();
            bytes.reserve(text.len() + 1); // text.len() + newline
            let end = bytes.as_mut_ptr().add(bytes.len());
            core::ptr::copy_nonoverlapping(text.as_ptr(), end, text.len());
            end.add(text.len()).write(b'\n');
            bytes.set_len(bytes.len() + text.len() + usize::from(append_newline));
        }
        true
    }
    /// Formats the `Data` into the given output (it's only used for printing right now)
    #[inline(never)]
    pub fn format_to(
        self,
        output: &mut String,
        obj_pool: &ObjectPool,
        str_pool: &StringPool,
        map_pool: &MapPool,
        structs: &[Struct],
        show_str_quotation_marks: bool,
    ) {
        if show_str_quotation_marks && self.is_string() {
            output.push('"');
            output.push_str(self.as_str(str_pool));
            output.push('"');
        } else if self.format_scalar_to(output, str_pool, false) {
        } else if self.is_array() {
            output.push('[');
            let mut show_comma = false;
            for value in &obj_pool[self.as_array()] {
                if show_comma {
                    output.push(',');
                } else {
                    show_comma = true;
                }
                value.format_to(output, obj_pool, str_pool, map_pool, structs, true);
            }
            output.push(']');
        } else if self.is_struct() {
            output.push_str(unsafe { structs.get_unchecked(self.struct_type_id() as usize).name });
            output.push_str(" {");
            if let Some((first, rest)) = obj_pool[self.as_struct()].split_first() {
                if first.is_string() {
                    output.push('"');
                    output.push_str(first.as_str(str_pool));
                    output.push('"');
                } else if !first.format_scalar_to(output, str_pool, false) {
                    first.format_to(output, obj_pool, str_pool, map_pool, structs, true);
                }
                for value in rest {
                    output.push(',');
                    if value.is_string() {
                        output.push('"');
                        output.push_str(value.as_str(str_pool));
                        output.push('"');
                    } else if !value.format_scalar_to(output, str_pool, false) {
                        value.format_to(output, obj_pool, str_pool, map_pool, structs, true);
                    }
                }
            }
            output.push('}');
        } else {
            debug_assert!(self.is_map());
            output.push('{');
            let mut show_comma = false;
            for (key, value) in &map_pool[self.as_map()] {
                if show_comma {
                    output.push(',');
                } else {
                    show_comma = true;
                }
                key.format_to(output, obj_pool, str_pool, map_pool, structs, true);
                output.push(':');
                value.format_to(output, obj_pool, str_pool, map_pool, structs, true);
            }
            output.push('}');
        }
    }
    #[inline(never)]
    pub fn matches_datatype(self, t: &DataType, obj_pool: &ObjectPool, map_pool: &MapPool) -> bool {
        match t {
            DataType::Int => self.is_int(),
            DataType::Float => self.is_float(),
            DataType::Bool => self.is_bool(),
            DataType::String => self.is_string(),
            DataType::Null => self.is_null(),
            DataType::Struct(id) => self.is_struct() && self.struct_type_id() == *id,
            DataType::Fn(_) | DataType::FnSignature(_) => self.is_function(),
            DataType::Union(types) => {
                types.iter().any(|x| self.matches_datatype(x, obj_pool, map_pool))
            }
            DataType::Array(None) => self.is_array(),
            DataType::Array(Some(elem_type)) => {
                self.is_array() && {
                    let array = &obj_pool[self.as_array()];
                    if matches!(**elem_type, DataType::Union(_)) {
                        array.iter().all(|e| e.matches_datatype(elem_type, obj_pool, map_pool))
                    } else {
                        array
                            .first()
                            .is_none_or(|e| e.matches_datatype(elem_type, obj_pool, map_pool))
                    }
                }
            }
            DataType::Map(kv) => {
                self.is_map() && {
                    map_pool[self.as_map()].iter().next().is_none_or(|(k, v)| {
                        kv.0.as_ref().is_none_or(|t| k.matches_datatype(t, obj_pool, map_pool))
                            && kv
                                .1
                                .as_ref()
                                .is_none_or(|t| v.matches_datatype(t, obj_pool, map_pool))
                    })
                }
            }
            DataType::Unknown => true,
        }
    }
}

impl From<f64> for Data {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Self::float(value)
    }
}
impl From<Data> for f64 {
    #[inline(always)]
    fn from(value: Data) -> Self {
        value.as_float()
    }
}

impl From<bool> for Data {
    #[inline(always)]
    fn from(value: bool) -> Self {
        Self(NAN_BOOL | u64::from(value))
    }
}
