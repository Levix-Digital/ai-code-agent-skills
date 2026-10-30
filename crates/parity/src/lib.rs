//! Parity harness: runs a Python script and its Rust translation on the same fixture and
//! compares everything the user can observe: stdout, stderr, exit code and the files written.
//!
//! A case (`tests/parity/cases/*.json`) names the Python script, the Rust command once the script
//! is translated, the arguments and a fixture. While the Python script exists it is the reference;
//! once it is deleted, the golden file recorded from it (`tests/parity/golden/<name>.json`) is.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Fixed identity and dates for fixture commits, so commit hashes are the same on every run.
const GIT_ENV: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "Parity"),
    ("GIT_AUTHOR_EMAIL", "parity@example.com"),
    ("GIT_COMMITTER_NAME", "Parity"),
    ("GIT_COMMITTER_EMAIL", "parity@example.com"),
    ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z"),
    ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z"),
];

/// Where the fake CLI finds its answers and writes its log.
pub const FAKE_DIR: &str = "PARITY_FAKE_DIR";

/// A canned answer of the fake CLI: the first one whose `match` is part of the command line
/// (the arguments joined with spaces) is given.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FakeAnswer {
    #[serde(rename = "match")]
    pattern: String,
    #[serde(default)]
    code: i32,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
}

/// The fake CLI, when this binary runs under another name (`gh`, `glab`): logs the call (the
/// arguments, and stdin when `--input -` asks for it) and answers from the case's file.
pub fn fake_cli(name: &str, args: &[String]) -> i32 {
    let Some(dir) = std::env::var_os(FAKE_DIR).map(PathBuf::from) else {
        eprintln!("{name}: {FAKE_DIR} is not set");
        return 1;
    };
    let reads_stdin = args.windows(2).any(|w| w[0] == "--input" && w[1] == "-");
    let mut input = String::new();
    if reads_stdin {
        let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut input);
    }
    let mut entry = serde_json::Map::new();
    entry.insert("args".into(), serde_json::json!(args));
    if reads_stdin {
        entry.insert("stdin".into(), serde_json::json!(input));
    }
    let line = serde_json::Value::Object(entry).to_string() + "\n";
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{name}.log")))
        .and_then(|mut f| f.write_all(line.as_bytes()));
    let answers: Vec<FakeAnswer> = fs::read_to_string(dir.join(format!("{name}.json")))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let command = args.join(" ");
    match answers.iter().find(|a| command.contains(&a.pattern)) {
        Some(a) => {
            print!("{}", a.stdout);
            eprint!("{}", a.stderr);
            a.code
        }
        None => {
            eprintln!("{name}: no answer for: {command}");
            1
        }
    }
}

/// Never compared: Git internals and Python bytecode caches.
const ALWAYS_IGNORED: [&str; 2] = [".git/", "__pycache__/"];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub name: String,
    /// Python script, relative to the repository root.
    pub python: String,
    /// Rust command (`["<skill>", "<script>"]`), once the script is translated.
    #[serde(default)]
    pub rust: Option<Vec<String>>,
    /// Arguments; `{project}`, `{home}` and `{skills}` are replaced with the run's paths.
    #[serde(default)]
    pub args: Vec<String>,
    /// Directory under `tests/parity/fixtures` copied as the project; empty when absent.
    #[serde(default)]
    pub fixture: Option<String>,
    /// Make the project a Git repository with one commit of the fixture.
    #[serde(default)]
    pub git: bool,
    /// With `git`: the URL of the repository's `origin` remote.
    #[serde(default)]
    pub remote: Option<String>,
    /// Programs replaced by the fake CLI (`gh`, `glab`, `gedo`): it answers from
    /// `<project>/.fake/<name>.json` and logs every call to `<project>/.fake/<name>.log`.
    #[serde(default)]
    pub fakes: Vec<String>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Regular expressions replaced with `<MASK>` before comparing (clocks, random ids), in
    /// the output, in the files and in the files' paths.
    #[serde(default)]
    pub masks: Vec<String>,
    /// Path prefixes (`project/...`, `home/...`) left out of the file comparison.
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Relative paths (regular expressions) the scripts print with `\\` on Windows; golden
    /// comparisons fold their separators to `/`, as they do for the run's absolute paths.
    #[serde(default)]
    pub seps: Vec<String>,
    /// Paths under the real home copied into the run's home first (the embedding model's
    /// cache, so no run downloads it); list them in `ignore` as well.
    #[serde(default)]
    pub seed_home: Vec<String>,
}

/// What a run leaves behind, normalized: paths of the run replaced with placeholders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
    /// Files created or changed (`Some(content)`) and deleted (`None`), keyed by
    /// `project/<path>` or `home/<path>`.
    pub files: BTreeMap<String, Option<String>>,
}

pub struct Harness {
    pub repo: PathBuf,
    pub python: String,
    pub rust_bin: PathBuf,
    pub work: PathBuf,
}

impl Harness {
    pub fn parity_dir(&self) -> PathBuf {
        self.repo.join("tests").join("parity")
    }

    pub fn golden_path(&self, case: &Case) -> PathBuf {
        self.parity_dir()
            .join("golden")
            .join(format!("{}.json", case.name))
    }

    pub fn load_cases(&self) -> Result<Vec<Case>, String> {
        let dir = self.parity_dir().join("cases");
        let mut files: Vec<PathBuf> = match fs::read_dir(&dir) {
            Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
            Err(_) => return Ok(Vec::new()),
        };
        files.retain(|p| p.extension().is_some_and(|e| e == "json"));
        files.sort();
        let mut cases = Vec::new();
        let mut names = BTreeSet::new();
        for f in files {
            let text = fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
            let batch: Vec<Case> =
                serde_json::from_str(&text).map_err(|e| format!("{}: {e}", f.display()))?;
            for c in batch {
                if !names.insert(c.name.clone()) {
                    return Err(format!("{}: duplicate case name '{}'", f.display(), c.name));
                }
                cases.push(c);
            }
        }
        Ok(cases)
    }

    pub fn python_exists(&self, case: &Case) -> bool {
        self.repo.join(&case.python).is_file()
    }

    pub fn run_python(&self, case: &Case) -> Result<Outcome, String> {
        let script = self.repo.join(&case.python);
        self.run(
            case,
            "py",
            &self.python,
            &[script.to_string_lossy().into_owned()],
        )
    }

    pub fn run_rust(&self, case: &Case) -> Result<Outcome, String> {
        let cmd = case.rust.as_ref().ok_or("case has no Rust command")?;
        self.run(case, "rs", &self.rust_bin.to_string_lossy(), cmd)
    }

    fn run(
        &self,
        case: &Case,
        tag: &str,
        program: &str,
        prefix: &[String],
    ) -> Result<Outcome, String> {
        let root = self.work.join(format!("{}-{tag}", case.name));
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
        }
        let project = root.join("project");
        let home = root.join("home");
        fs::create_dir_all(&project).map_err(|e| e.to_string())?;
        fs::create_dir_all(&home).map_err(|e| e.to_string())?;
        if let Some(fx) = &case.fixture {
            copy_dir(&self.parity_dir().join("fixtures").join(fx), &project)?;
        }
        if case.git {
            git_init(&project, &home)?;
            if let Some(url) = &case.remote {
                git(&project, &home, &["remote", "add", "origin", url])?;
            }
        }
        let fake_bin = root.join("bin");
        if !case.fakes.is_empty() {
            fs::create_dir_all(&fake_bin).map_err(|e| e.to_string())?;
            let me = std::env::current_exe().map_err(|e| e.to_string())?;
            for name in &case.fakes {
                let target = fake_bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
                fs::copy(&me, &target).map_err(|e| format!("{}: {e}", target.display()))?;
            }
        }
        let real_home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
            .map(PathBuf::from)
            .ok_or("no home directory")?;
        for rel in &case.seed_home {
            let from = real_home.join(rel);
            if !from.is_dir() {
                return Err(format!("seed_home: {} is missing", from.display()));
            }
            let to = home.join(rel);
            fs::create_dir_all(&to).map_err(|e| e.to_string())?;
            copy_dir(&from, &to)?;
        }
        // Canonical paths: the scripts print resolved paths (macOS /private/var, Windows \\?\ aside).
        let project = canonical(&project);
        let home = canonical(&home);
        let skills = canonical(&self.repo);
        let before = snapshot(&root)?;

        let fill = |s: &str| {
            s.replace("{project}", &project.to_string_lossy())
                .replace("{home}", &home.to_string_lossy())
                .replace("{skills}", &skills.to_string_lossy())
        };
        let mut command = Command::new(program);
        command
            .args(prefix)
            .args(case.args.iter().map(|a| fill(a)))
            .current_dir(&project)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .envs(GIT_ENV)
            .envs(case.env.iter().map(|(k, v)| (k.clone(), fill(v))));
        if !case.fakes.is_empty() {
            let mut dirs = vec![fake_bin.clone()];
            if let Some(path) = std::env::var_os("PATH") {
                dirs.extend(std::env::split_paths(&path));
            }
            command
                .env(
                    "PATH",
                    std::env::join_paths(dirs).map_err(|e| e.to_string())?,
                )
                .env(FAKE_DIR, project.join(".fake"));
        }
        command
            .stdin(if case.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|e| format!("{program}: {e}"))?;
        if let Some(input) = &case.stdin {
            let mut pipe = child.stdin.take().ok_or("no stdin")?;
            pipe.write_all(input.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;

        let after = snapshot(&root)?;
        let masks = case
            .masks
            .iter()
            .map(|m| Regex::new(m).map_err(|e| format!("mask '{m}': {e}")))
            .collect::<Result<Vec<_>, _>>()?;
        let norm = Normalizer {
            project,
            home,
            skills,
            masks,
        };
        let mut files = BTreeMap::new();
        for (path, content) in &after {
            if before.get(path) != Some(content) && !ignored(&norm.apply(path), &case.ignore) {
                files.insert(norm.apply(path), Some(norm.apply(&text(content, path)?)));
            }
        }
        for path in before.keys() {
            if !after.contains_key(path) {
                files.insert(norm.apply(path), None);
            }
        }
        files.retain(|p, _| !ignored(p, &case.ignore));
        Ok(Outcome {
            exit: out.status.code().unwrap_or(-1),
            stdout: norm.apply(&text(&out.stdout, "stdout")?),
            stderr: norm.apply(&text(&out.stderr, "stderr")?),
            files,
        })
    }
}

pub struct Normalizer {
    pub project: PathBuf,
    pub home: PathBuf,
    pub skills: PathBuf,
    pub masks: Vec<Regex>,
}

impl Normalizer {
    /// Run paths become `{project}`, `{home}` and `{skills}` (also with forward slashes, as some
    /// scripts print them on Windows); then the case's masks apply.
    pub fn apply(&self, s: &str) -> String {
        let mut out = s.to_string();
        for (path, label) in [
            (&self.project, "{project}"),
            (&self.home, "{home}"),
            (&self.skills, "{skills}"),
        ] {
            let p = path.to_string_lossy().into_owned();
            out = out.replace(&p, label);
            let fwd = p.replace('\\', "/");
            if fwd != p {
                out = out.replace(&fwd, label);
            }
        }
        for m in &self.masks {
            out = m.replace_all(&out, "<MASK>").into_owned();
        }
        out
    }
}

/// Python writes `\r\n` and `\\` path separators on Windows; golden files are recorded with `\n`
/// and `/`. Only golden comparisons fold them: Python and Rust on the same machine must match
/// byte for byte. `seps` are the case's relative paths to fold as well.
pub fn fold_newlines(o: &Outcome, seps: &[String]) -> Outcome {
    let mut paths = vec![Regex::new(r#"\{(?:project|home|skills)\}[^\s"'`]*"#).expect("valid")];
    paths.extend(
        seps.iter()
            .map(|s| Regex::new(s).expect("valid seps pattern")),
    );
    let f = |s: &str| {
        let mut s = s.replace("\r\n", "\n");
        for p in &paths {
            s = p
                .replace_all(&s, |c: &regex::Captures| {
                    c[0].replace("\\\\", "/").replace('\\', "/")
                })
                .into_owned();
        }
        s
    };
    Outcome {
        exit: o.exit,
        stdout: f(&o.stdout),
        stderr: f(&o.stderr),
        files: o
            .files
            .iter()
            .map(|(k, v)| (k.clone(), v.as_deref().map(f)))
            .collect(),
    }
}

/// Differences between a reference and a candidate, one line each, empty when identical.
pub fn diff(reference: &Outcome, candidate: &Outcome) -> Vec<String> {
    let mut out = Vec::new();
    if reference.exit != candidate.exit {
        out.push(format!(
            "exit code: {} != {}",
            reference.exit, candidate.exit
        ));
    }
    for (label, a, b) in [
        ("stdout", &reference.stdout, &candidate.stdout),
        ("stderr", &reference.stderr, &candidate.stderr),
    ] {
        if let Some(d) = first_difference(a, b) {
            out.push(format!("{label}: {d}"));
            // A different stderr is usually a traceback or an error message: show it whole.
            if label == "stderr" {
                out.extend(b.lines().take(40).map(|l| format!("  | {l}")));
            }
        }
    }
    let paths: BTreeSet<&String> = reference
        .files
        .keys()
        .chain(candidate.files.keys())
        .collect();
    for p in paths {
        match (reference.files.get(p), candidate.files.get(p)) {
            (Some(a), Some(b)) if a == b => {}
            (Some(Some(a)), Some(Some(b))) => out.push(format!(
                "{p}: {}",
                first_difference(a, b).unwrap_or_default()
            )),
            (a, b) => out.push(format!("{p}: {} != {}", describe(a), describe(b))),
        }
    }
    out
}

fn describe(f: Option<&Option<String>>) -> &'static str {
    match f {
        None => "unchanged",
        Some(None) => "deleted",
        Some(Some(_)) => "written",
    }
}

fn first_difference(a: &str, b: &str) -> Option<String> {
    if a == b {
        return None;
    }
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.split('\n').collect(), b.split('\n').collect());
    let i = la
        .iter()
        .zip(&lb)
        .position(|(x, y)| x != y)
        .unwrap_or(la.len().min(lb.len()));
    Some(format!(
        "line {}: {:?} != {:?}",
        i + 1,
        la.get(i).copied().unwrap_or("<end>"),
        lb.get(i).copied().unwrap_or("<end>")
    ))
}

fn ignored(path: &str, extra: &[String]) -> bool {
    let rel = path.split_once('/').map_or(path, |(_, r)| r);
    ALWAYS_IGNORED
        .iter()
        .any(|p| rel.starts_with(p) || rel.contains(&format!("/{p}")))
        || extra.iter().any(|p| path.starts_with(p.as_str()))
}

fn text(bytes: &[u8], what: &str) -> Result<String, String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| format!("{what}: not UTF-8"))
}

/// Resolved path without the Windows verbatim prefix (`\\?\`), which breaks the `..` joins the
/// Python scripts make from `__file__`.
pub fn canonical(p: &Path) -> PathBuf {
    let c = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    // Windows: drop the verbatim prefix, which no script prints.
    let s = c.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => c,
    }
}

/// Every file under `root`, keyed by `project/...` or `home/...` with forward slashes.
fn snapshot(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                // Never compared, and Git may still be changing it in the background.
                if path.file_name() != Some(".git".as_ref()) {
                    stack.push(path);
                }
            } else {
                let rel = path.strip_prefix(root).map_err(|e| e.to_string())?;
                let key = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(key, fs::read(&path).map_err(|e| e.to_string())?);
            }
        }
    }
    Ok(out)
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    for entry in fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let target = to.join(path.file_name().ok_or("bad file name")?);
        if path.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn git(project: &Path, home: &Path, args: &[&str]) -> Result<(), String> {
    let st = Command::new("git")
        .args(args)
        .current_dir(project)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .envs(GIT_ENV)
        .status()
        .map_err(|e| format!("git: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("git {}: failed", args.join(" ")))
    }
}

fn git_init(project: &Path, home: &Path) -> Result<(), String> {
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.name", "Parity"],
        &["config", "user.email", "parity@example.com"],
        &["config", "core.autocrlf", "false"],
        &["add", "-A"],
        &["commit", "-q", "--allow-empty", "-m", "fixture"],
    ] {
        git(project, home, args)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(stdout: &str) -> Outcome {
        Outcome {
            exit: 0,
            stdout: stdout.into(),
            stderr: String::new(),
            files: BTreeMap::new(),
        }
    }

    #[test]
    fn golden_comparisons_fold_windows_paths() {
        let o = Outcome {
            exit: 0,
            stdout: "Source: {project}\\docs\\a.md\r\nC:\\other\r\n".into(),
            stderr: String::new(),
            files: BTreeMap::new(),
        };
        assert_eq!(
            fold_newlines(&o, &[]).stdout,
            "Source: {project}/docs/a.md\nC:\\other\n"
        );
    }

    #[test]
    fn golden_comparisons_fold_the_case_relative_paths() {
        let seps = [r"docs\\adr".to_string()];
        assert_eq!(
            fold_newlines(&outcome("(docs\\adr, a\\b)\r\n"), &seps).stdout,
            "(docs/adr, a\\b)\n"
        );
    }

    #[test]
    fn folds_json_escaped_separators() {
        let seps = [r"docs(?:\\\\|\\)adr".to_string()];
        assert_eq!(
            fold_newlines(&outcome("\"docs\\\\adr\"\n"), &seps).stdout,
            "\"docs/adr\"\n"
        );
    }

    #[test]
    fn identical_outcomes_have_no_difference() {
        assert!(diff(&outcome("a\nb\n"), &outcome("a\nb\n")).is_empty());
    }

    #[test]
    fn reports_the_first_different_line() {
        let d = diff(&outcome("a\nb\n"), &outcome("a\nc\n"));
        assert_eq!(d, vec![r#"stdout: line 2: "b" != "c""#]);
    }

    #[test]
    fn reports_exit_code_and_files() {
        let mut a = outcome("");
        let mut b = outcome("");
        b.exit = 1;
        a.files.insert("project/x.md".into(), Some("1".into()));
        b.files.insert("project/y.md".into(), None);
        let d = diff(&a, &b);
        assert_eq!(d[0], "exit code: 0 != 1");
        assert!(d.contains(&"project/x.md: written != unchanged".to_string()));
        assert!(d.contains(&"project/y.md: unchanged != deleted".to_string()));
    }

    #[test]
    fn normalizes_paths_and_masks() {
        let n = Normalizer {
            project: PathBuf::from("/tmp/run/project"),
            home: PathBuf::from("/tmp/run/home"),
            skills: PathBuf::from("/repo"),
            masks: vec![Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap()],
        };
        assert_eq!(
            n.apply("/tmp/run/project/a /tmp/run/home/b /repo/c 2026-10-27"),
            "{project}/a {home}/b {skills}/c <MASK>"
        );
    }

    #[test]
    fn folds_windows_newlines_only_for_golden_comparisons() {
        assert_eq!(
            fold_newlines(&outcome("a\r\nb\r\n"), &[]),
            outcome("a\nb\n")
        );
        assert!(!diff(&outcome("a\r\n"), &outcome("a\n")).is_empty());
    }

    #[test]
    fn ignores_git_and_caches() {
        assert!(ignored("project/.git/HEAD", &[]));
        assert!(ignored("project/plan-memory/__pycache__/x.pyc", &[]));
        assert!(ignored(
            "home/.ai-code-agent-memory/db/x",
            &["home/.ai-code-agent-memory/".into()]
        ));
        assert!(!ignored("project/docs/BACKLOG.md", &[]));
    }
}
