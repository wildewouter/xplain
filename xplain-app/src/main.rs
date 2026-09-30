//! Binary entry point. Only calls [`xplain_app::run::main_with_args`] and exits.
//! Must not contain logic.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = xplain_app::run::main_with_args(&args);
    std::process::exit(code);
}
