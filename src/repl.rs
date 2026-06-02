use inline_colorization::*;
use std::io::{Write, stdin, stdout};

pub fn repl() {
    println!(
        "{color_blue}KEEL {} -- REPL (read-eval-print-loop){color_reset}",
        env!("CARGO_PKG_VERSION")
    );

    let exe = std::env::current_exe()
        .expect("{color_red}[ERROR]{color_reset} Cannot find keel binary path");
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
            .expect("{color_red}[ERROR]{color_reset} Incorrect input string");
        if let Some('\n') = s.chars().next_back() {
            s.pop();
        }
        if let Some('\r') = s.chars().next_back() {
            s.pop();
        }
        if s.is_empty() {
            continue;
        }
        if !s.ends_with(';') && !s.ends_with('}') {
            s.push(';');
        }
        if s.contains("exit()") && !s.contains('"') {
            println!("{color_blue}[KEEL TIP]{color_reset} To exit, press Ctrl+C")
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
            .expect("{color_red}[ERROR]{color_reset} Cannot write to temporary file");

        let output = std::process::Command::new(&exe)
            .arg(tmp.to_str().unwrap())
            .output()
            .expect("{color_red}[ERROR]{color_reset} Failed to execute Keel");

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
