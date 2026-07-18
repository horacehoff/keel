use crate::errors::BLUE;
use crate::errors::RESET;
use std::io::{Write, stdin, stdout};

#[cold]
#[inline(never)]
pub fn repl() {
    println!(
        "{BLUE}KEEL {} -- REPL (read-eval-print-loop){RESET}",
        env!("CARGO_PKG_VERSION")
    );

    let exe = std::env::current_exe().expect("{RED}[ERROR]{RESET} Cannot find keel binary path");
    let tmp = std::env::temp_dir().join("keel_repl_tmp.kl");

    let mut all_lines: Vec<String> = Vec::with_capacity(1);
    let mut prev_stdout = String::with_capacity(1);
    let mut contents = String::with_capacity(20);

    loop {
        let mut s = String::new();
        print!("> ");
        let _ = stdout().flush();
        stdin()
            .read_line(&mut s)
            .expect("{RED}[ERROR]{RESET} Incorrect input string");
        if s.ends_with('\n') {
            s.pop();
        }
        if s.ends_with('\r') {
            s.pop();
        }
        if s.is_empty() {
            continue;
        }
        if !s.ends_with(';') && !s.ends_with('}') {
            s.push(';');
        }
        if s.contains("exit()") && !s.contains('"') {
            println!("{BLUE}[KEEL TIP]{RESET} To exit, press Ctrl+C");
        }

        all_lines.push(s);

        contents.clear();
        for x in all_lines.iter().filter(|x| x.starts_with("import")) {
            contents.push_str(x);
            contents.push('\n');
        }
        contents.push_str("fn main() {\n");
        for x in all_lines.iter().filter(|x| !x.starts_with("import")) {
            contents.push_str(x);
            contents.push('\n');
        }
        contents.push('\n');
        contents.push('}');

        std::fs::write(&tmp, &contents)
            .expect("{RED}[ERROR]{RESET} Cannot write to temporary file");

        let output = std::process::Command::new(&exe)
            .arg(tmp.to_str().unwrap())
            .output()
            .expect("{RED}[ERROR]{RESET} Failed to execute Keel");

        std::fs::remove_file(&tmp).unwrap();

        let new_stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if output.status.success() {
            if new_stdout.len() > prev_stdout.len() {
                print!("{}", &new_stdout[prev_stdout.len()..]);
            }
            prev_stdout = new_stdout;
        } else {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            all_lines.pop();
        }
    }
}
