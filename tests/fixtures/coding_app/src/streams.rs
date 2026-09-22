use std::io::Write;

pub struct Console;

impl Console {
    pub fn print_all(&self) {
        println!("to stdout");
        print!("no newline");
        eprintln!("to stderr");
        eprint!("no newline either");
        let value = dbg!(42);
        let _ = value;
    }

    pub fn write_directly(&self) -> std::io::Result<()> {
        std::io::stdout().write_all(b"raw")?;
        let mut err = std::io::stderr();
        err.write_all(b"raw")?;
        Ok(())
    }

    pub fn quiet(&self) -> String {
        format!("{}", 1)
    }
}
