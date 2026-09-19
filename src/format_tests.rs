use lexical_core::FormattedSize;

pub trait PushStrUnchecked {
    fn push_str_unchecked(&mut self, string: &str);
}

impl PushStrUnchecked for String {
    #[inline]
    fn push_str_unchecked(&mut self, string: &str) {
        let len = self.len();
        let string_len = string.len();
        debug_assert!(string_len <= self.capacity() - len);
        unsafe {
            std::ptr::copy_nonoverlapping(string.as_ptr(), self.as_mut_ptr().add(len), string_len);
            self.as_mut_vec().set_len(len + string_len);
        }
    }
}

pub trait HybridFormat {
    fn size(&self) -> usize;
    fn append(&self, buf: &mut String);
}

impl HybridFormat for str {
    #[inline]
    fn size(&self) -> usize {
        self.len()
    }
    #[inline]
    fn append(&self, buf: &mut String) {
        buf.push_str_unchecked(self);
    }
}
impl HybridFormat for bool {
    #[inline]
    fn size(&self) -> usize {
        5
    }
    #[inline]
    fn append(&self, buf: &mut String) {
        buf.push_str_unchecked(if *self { "true" } else { "false" });
    }
}
impl HybridFormat for f64 {
    #[inline]
    fn size(&self) -> usize {
        24
    }
    #[inline]
    fn append(&self, buf: &mut String) {
        buf.push_str_unchecked(zmij::Buffer::new().format(*self));
    }
}
macro_rules! HybridFormatInt {
    ($($t: ty )*) => {$(
        impl HybridFormat for $t {
            #[inline]
            fn size(&self) -> usize {
                Self::FORMATTED_SIZE_DECIMAL
            }
            #[inline]
            fn append(&self, buf: &mut String) {
                let mut buffer = [0u8; Self::FORMATTED_SIZE_DECIMAL];
                let digits = lexical_core::write(*self, &mut buffer);
                buf.push_str_unchecked(unsafe { str::from_utf8_unchecked(digits) });
            }
        })*
    };
}
HybridFormatInt!(i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize);

// a VERY experimental *idea* for a hybrid format macro
// the basic idea is to build the formatted string / static str recursively through the macro
// This might end up as a separate lib if it's good enough!
// this is NOT pretty
#[macro_export]
macro_rules! hformat {
    // a dynamic (runtime) item with some elements after
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] [$($add_to_str_statements: tt)*] {$dynamic_elem: expr}, $($remaining:tt)*) => {{
        let _temp_formatted: &str = const_format::concatcp!($($pending_static_elems)*);
        hformat!(
            [$buffer]
            []
            [$($capacity_expr)* + _temp_formatted.len() + $dynamic_elem.size()]
            [$(
                $add_to_str_statements)*
                $buffer.push_str_unchecked(_temp_formatted);
                ($dynamic_elem).append($buffer);
            ]
            $($remaining)*
        )
    }};
    // a dynamic (runtime) item with no elements after (the last one), allows a trailing comma
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] [$($add_to_str_statements: tt)*] {$dynamic_elem: expr} $(,)?) => {{
        let _temp_formatted: &str = const_format::concatcp!($($pending_static_elems)*);
        hformat!(
            [$buffer]
            []
            [$($capacity_expr)* + _temp_formatted.len() + $dynamic_elem.size()]
            [$(
                $add_to_str_statements)*
                $buffer.push_str_unchecked(_temp_formatted);
                ($dynamic_elem).append($buffer);
            ]
        )
    }};
    // static item with some elements after
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] [$($add_to_str_statements: tt)*] $static_elem: expr, $($remaining:tt)*) => {{
        hformat!(
            [$buffer]
            [$($pending_static_elems)* $static_elem,]
            [$($capacity_expr)*]
            [$($add_to_str_statements)*]
            $($remaining)*
        )
    }};
    // static item with no elements after
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] [$($add_to_str_statements: tt)*] $static_elem: expr $(,)?) => {{
        hformat!(
            [$buffer]
            [$($pending_static_elems)* $static_elem,]
            [$($capacity_expr)*]
            [$($add_to_str_statements)*]
        )
    }};
    // pure const
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] []) => {
        const_format::concatcp!($($pending_static_elems)*)
    };
    // runtime/const hybrid
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] [$($add_to_str_statements: tt)*]) => {{
        #[allow(unused_imports)]
        use $crate::format_tests::PushStrUnchecked;
        #[allow(unused_imports)]
        use $crate::format_tests::HybridFormat;
        let _temp_formatted: &str = const_format::concatcp!($($pending_static_elems)*);
        let mut $buffer = String::with_capacity($($capacity_expr)* + _temp_formatted.len());
        {
            // shadowed just to give the statements a &mut String
            let $buffer = &mut $buffer;
            $($add_to_str_statements)*
        }
        $buffer.push_str_unchecked(_temp_formatted);
        $buffer
    }};
    // the last one, it's the one that's actually called in the code
    ($($elems: tt)*) => {
        hformat!(
            [buf]
            []
            [0usize]
            []
            $($elems)*
        )
    };
}
