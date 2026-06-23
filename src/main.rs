//! Thin binary entry point: parse args, dispatch, set the process exit code.

fn main() {
    std::process::exit(ai_meta::cli::run());
}
