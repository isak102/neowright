use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(neowright::run(std::env::args_os()) as u8)
}
