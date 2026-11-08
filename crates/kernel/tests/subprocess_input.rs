//! `run_replace_input`: the text reaches the program's standard input.

use kernel::py::subprocess::{run_replace, run_replace_input};

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn input_reaches_standard_input() {
    // `git hash-object --stdin` hashes what it reads: the blob hash of "y\n".
    let (code, stdout, _) =
        run_replace_input(&args(&["git", "hash-object", "--stdin"]), None, Some("y\n"))
            .expect("git runs");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "975fbec8256d3e8a3797e7a3611380f27c49f4ac");
}

#[test]
fn no_input_is_an_empty_standard_input() {
    let (code, stdout, _) =
        run_replace(&args(&["git", "hash-object", "--stdin"]), None).expect("git runs");
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
}
