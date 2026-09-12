use lexical_core::FormattedSize;

pub trait TestFormat {
    fn size(&self) -> usize;
    fn append(&self, buf: &mut String);
}

impl TestFormat for str {
    #[inline]
    fn size(&self) -> usize {
        self.len()
    }
    #[inline]
    fn append(&self, buf: &mut String) {
        buf.push_str(self);
    }
}
impl TestFormat for bool {
    #[inline]
    fn size(&self) -> usize {
        5
    }
    #[inline]
    fn append(&self, buf: &mut String) {
        buf.push_str(if *self { "true" } else { "false" });
    }
}
macro_rules! TestFormatInt {
    ($($t: ty )*) => {$(
        impl TestFormat for $t {
            #[inline]
            fn size(&self) -> usize {
                Self::FORMATTED_SIZE_DECIMAL
            }
            #[inline]
            fn append(&self, buf: &mut String) {
                let mut buffer = [0u8; Self::FORMATTED_SIZE_DECIMAL];
                let digits = lexical_core::write(*self, &mut buffer);
                buf.push_str(unsafe { str::from_utf8_unchecked(digits) });
            }
        })*
    };
}
TestFormatInt!(i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize);
impl TestFormat for f64 {
    #[inline]
    fn size(&self) -> usize {
        24
    }
    #[inline]
    fn append(&self, buf: &mut String) {
        buf.push_str(zmij::Buffer::new().format(*self));
    }
}

#[macro_export]
// a VERY experimental *idea* for a hybrid format macro
// the basic idea is to build the formatted string / static str recursively through the macro
// This might end up as a separate lib if it's good enough!
// this is NOT pretty
macro_rules! hformat {
    // a dynamic (runtime) item with some elements after
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] [$($add_to_str_statements: tt)*] {$dynamic_elem: expr}, $($remaining:tt)*) => {{
        const _TEMP_FORMATTED: &str = const_format::concatcp!($($pending_static_elems)*);
        hformat!(
            [$buffer]
            []
            [$($capacity_expr)* + _TEMP_FORMATTED.len() + $dynamic_elem.size()]
            [$(
                $add_to_str_statements)*
                $buffer.push_str(_TEMP_FORMATTED);
                ($dynamic_elem).append($buffer);
            ]
            $($remaining)*
        )
    }};
    // a dynamic (runtime) item with no elements after (the last one), allows a trailing comma
    ([$buffer:ident] [$($pending_static_elems: tt)*] [$($capacity_expr: tt)*] [$($add_to_str_statements: tt)*] {$dynamic_elem: expr} $(,)?) => {{
        const _TEMP_FORMATTED: &str = const_format::concatcp!($($pending_static_elems)*);
        hformat!(
            [$buffer]
            []
            [$($capacity_expr)* + _TEMP_FORMATTED.len() + $dynamic_elem.size()]
            [$(
                $add_to_str_statements)*
                $buffer.push_str(_TEMP_FORMATTED);
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
        const _TEMP_FORMATTED: &str = const_format::concatcp!($($pending_static_elems)*);
        let mut $buffer = String::with_capacity($($capacity_expr)* + _TEMP_FORMATTED.len());
        {
            // shadowed just to give the statements a &mut String
            let $buffer = &mut $buffer;
            $($add_to_str_statements)*
        }
        $buffer.push_str(_TEMP_FORMATTED);
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

fn test() {
    let y = 3;
    let z = 9;
    let x = hformat!({ y }, { z }, "hey");
}
