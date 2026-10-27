//! Writing to stdout and stderr as Python's `print` does: UTF-8, and `\n` written as `\r\n` on
//! Windows (text mode). A closed pipe is ignored.

use std::io::Write;

fn translate(text: &str) -> std::borrow::Cow<'_, str> {
    if cfg!(windows) && text.contains('\n') {
        text.replace('\n', "\r\n").into()
    } else {
        text.into()
    }
}

/// Writes `text` to stdout as is (add the `\n` that `print` adds). Text Python could not
/// encode (a lone surrogate) ends the program as Python's uncaught exception did.
pub fn out(text: &str) {
    if let Err(crash) = crate::py::encode_check(text) {
        std::process::exit(crash.report().into());
    }
    let mut s = std::io::stdout().lock();
    let _ = s.write_all(translate(text).as_bytes());
    let _ = s.flush();
}

/// Writes `text` to stderr as is.
pub fn err(text: &str) {
    let mut s = std::io::stderr().lock();
    let _ = s.write_all(translate(text).as_bytes());
    let _ = s.flush();
}

/// `print(text)`.
pub fn println(text: &str) {
    out(&format!("{text}\n"));
}
