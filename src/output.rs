//! Color + logging helpers, mirroring the bash `meta_info/head/note/ok/warn/err`
//! API. Color is auto-disabled when stderr is not a TTY or `NO_COLOR` is set.

use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

static COLOR: AtomicBool = AtomicBool::new(true);

/// Initialize color state from the environment. Call once at startup.
pub fn init_color() {
    let enabled = std::env::var_os("NO_COLOR").is_none() && std::io::stderr().is_terminal();
    COLOR.store(enabled, Ordering::Relaxed);
}

/// Force color on/off (e.g. a `--no-color` flag or tests).
pub fn set_color(enabled: bool) {
    COLOR.store(enabled, Ordering::Relaxed);
}

fn color_on() -> bool {
    COLOR.load(Ordering::Relaxed)
}

fn paint(code: &str, s: &str) -> String {
    if color_on() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    paint("1", s)
}
pub fn dim(s: &str) -> String {
    paint("2", s)
}
pub fn red(s: &str) -> String {
    paint("31", s)
}
pub fn green(s: &str) -> String {
    paint("32", s)
}
pub fn yellow(s: &str) -> String {
    paint("33", s)
}
pub fn blue(s: &str) -> String {
    paint("34", s)
}

/// Plain line to stdout.
pub fn info(msg: impl AsRef<str>) {
    println!("{}", msg.as_ref());
}

/// Bold heading to stdout.
pub fn head(msg: impl AsRef<str>) {
    println!("{}", bold(msg.as_ref()));
}

/// Blue NOTE to stderr (advisory).
pub fn note(msg: impl AsRef<str>) {
    eprintln!("{} {}", blue("NOTE"), msg.as_ref());
}

/// Green OK to stdout.
pub fn ok(msg: impl AsRef<str>) {
    println!("{} {}", green("OK"), msg.as_ref());
}

/// Yellow WARN to stderr.
pub fn warn(msg: impl AsRef<str>) {
    eprintln!("{} {}", yellow("WARN"), msg.as_ref());
}

/// Red ERROR to stderr.
pub fn err(msg: impl AsRef<str>) {
    eprintln!("{} {}", red("ERROR"), msg.as_ref());
}
