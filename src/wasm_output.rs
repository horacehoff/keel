use std::cell::RefCell;

thread_local! {
    pub static WASM_OUTPUT: RefCell<String> = const {RefCell::new(String::new())};
}

pub fn print(s: &str) {
    WASM_OUTPUT.with(|o| o.borrow_mut().push_str(s));
}

pub struct WasmWriter;

impl std::io::Write for WasmWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(s) = std::str::from_utf8(buf) {
            print(s);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        print(&fmt.to_string());
        Ok(())
    }
}
