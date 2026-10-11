//! The `ohrsi` binary; see [`ohrsi::cli`] for the commands.

use std::process::ExitCode;

fn main() -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_default();
    let code = ohrsi::cli::run(
        std::env::args().skip(1),
        &cwd,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    ExitCode::from(code)
}
