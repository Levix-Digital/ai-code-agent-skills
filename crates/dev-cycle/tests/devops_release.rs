//! `check_ci_policy`, `release_status` and `branching` (formerly
//! `devops-release/tests/test_devops_release.py` and `test_branching.py`).

mod common;

use common::{Project, has, skills_root};
use devops_release::release_status::{
    evaluate_release, next_rc_tag, project_version, read_version_file,
};
use kernel::project_config::load_project_config;
use kernel::py::json::loads;

/// `.dev-cycle.json` with `devops`, plus workflows and files (added to what is already there).
fn repo(p: &Project, devops: &str, workflows: &[(&str, &str)], files: &[(&str, &str)]) {
    p.write(".dev-cycle.json", &format!(r#"{{"devops": {devops}}}"#));
    p.mkdir(".github/workflows");
    for (name, text) in workflows {
        p.write(&format!(".github/workflows/{name}"), text);
    }
    for (rel, text) in files {
        p.write(rel, text);
    }
}

fn template(name: &str) -> String {
    std::fs::read_to_string(skills_root().join("devops-release/templates").join(name))
        .expect("template")
}

/// The engine's check_ci_policy: (errors, warnings) from its report.
fn check_ci(p: &Project) -> (Vec<String>, Vec<String>) {
    let root = p.root();
    let res = p.run("devops-release", "check_ci_policy", &["--dir", &root]);
    let output = res.all();
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| !l.contains("devops-release CI policy:"))
        .collect();
    let errors: Vec<String> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("❌ ").map(str::to_string))
        .collect();
    let warnings: Vec<String> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("⚠️ ").map(str::to_string))
        .collect();
    assert_eq!(res.code, i32::from(!errors.is_empty()), "{output}");
    (errors, warnings)
}

fn any(items: &[String], piece: &str) -> bool {
    items.iter().any(|i| i.contains(piece))
}

const LINT_PLACEHOLDER: &str = r#"echo "replace with the stack's linters" && exit 1"#;
const TEST_PLACEHOLDER: &str = r#"echo "replace with the project's test_commands" && exit 1"#;

fn ci_for(branches: &str) -> String {
    template("ci.yml")
        .replace("branches: [dev, main]", &format!("branches: {branches}"))
        .replace(TEST_PLACEHOLDER, "pytest")
        .replace(LINT_PLACEHOLDER, "ruff check .")
}

#[test]
fn templates_pass_the_policy() {
    let p = Project::new();
    let ci = template("ci.yml")
        .replace(TEST_PLACEHOLDER, "cargo test --workspace")
        .replace(
            LINT_PLACEHOLDER,
            "cargo clippy --all-targets -- -D warnings",
        );
    repo(
        &p,
        r#"{"version_files": ["version.txt"]}"#,
        &[
            ("ci.yml", &ci),
            ("release.yml", &template("release.yml")),
            ("tag-on-merge.yml", &template("tag-on-merge.yml")),
        ],
        &[("version.txt", "1.2.3\n")],
    );
    let (errors, warnings) = check_ci(&p);
    assert!(
        errors.is_empty() && warnings.is_empty(),
        "{errors:?} {warnings:?}"
    );
}

#[test]
fn trunk_templates_pass_and_tag_on_merge_is_refused() {
    let p = Project::new();
    let devops = r#"{"branching": "trunk", "version_files": ["version.txt"]}"#;
    let flows = [
        ("ci.yml", ci_for("[main]")),
        ("release.yml", template("release.yml")),
        ("promote.yml", template("promote.yml")),
    ];
    let flows: Vec<(&str, &str)> = flows.iter().map(|(n, t)| (*n, t.as_str())).collect();
    repo(&p, devops, &flows, &[("version.txt", "1.2.3\n")]);
    assert_eq!(check_ci(&p), (vec![], vec![]));
    repo(
        &p,
        devops,
        &[("tag-on-merge.yml", &template("tag-on-merge.yml"))],
        &[],
    );
    let (errors, _) = check_ci(&p);
    assert!(
        any(&errors, "tag-on-merge.yml: creates a tag on every merge"),
        "{errors:?}"
    );
}

#[test]
fn trunk_needs_ci_only_into_main_and_a_promote_workflow() {
    let p = Project::new();
    repo(
        &p,
        r#"{"branching": "trunk"}"#,
        &[
            ("ci.yml", &ci_for("[main]")),
            ("release.yml", &template("release.yml")),
        ],
        &[],
    );
    let (errors, warnings) = check_ci(&p);
    assert!(!any(&errors, "pull requests into"), "{errors:?}");
    assert!(any(&warnings, "no promote workflow"), "{warnings:?}");
}

#[test]
fn promote_without_environment_is_warned() {
    let p = Project::new();
    let promote = template("promote.yml").replace("    environment: release\n", "");
    repo(
        &p,
        r#"{"branching": "trunk"}"#,
        &[
            ("ci.yml", &ci_for("[main]")),
            ("promote.yml", &promote),
            ("release.yml", &template("release.yml")),
        ],
        &[],
    );
    let (_, warnings) = check_ci(&p);
    assert!(
        any(&warnings, "promote.yml: no 'environment:'"),
        "{warnings:?}"
    );
}

#[test]
fn release_branches_need_ci_into_release_branches() {
    let p = Project::new();
    let (release, promote) = (template("release.yml"), template("promote.yml"));
    let devops = r#"{"branching": "release-branches"}"#;
    repo(
        &p,
        devops,
        &[
            ("release.yml", &release),
            ("promote.yml", &promote),
            ("ci.yml", &ci_for("[main]")),
        ],
        &[],
    );
    let (errors, _) = check_ci(&p);
    assert!(any(&errors, "pull requests into 'release/*'"), "{errors:?}");
    repo(
        &p,
        devops,
        &[("ci.yml", &ci_for("[main, 'release/**']"))],
        &[],
    );
    let (errors, _) = check_ci(&p);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn pull_requests_without_a_linter_are_warned() {
    let p = Project::new();
    let ci = template("ci.yml").replace(TEST_PLACEHOLDER, "pytest");
    repo(&p, "{}", &[("ci.yml", &ci)], &[]);
    let (errors, warnings) = check_ci(&p);
    assert!(!any(&errors, "linter"), "{errors:?}");
    assert!(
        any(
            &warnings,
            "no workflow runs a linter or static analysis on pull requests into 'dev'"
        ),
        "{warnings:?}"
    );
    let lint = "on:\n  pull_request:\n    branches: [dev, main]\njobs:\n  l:\n    runs-on: ubuntu-latest\n    steps:\n      - run: npm run lint\n";
    repo(&p, "{}", &[("ci.yml", &ci), ("lint.yml", lint)], &[]);
    let (_, warnings) = check_ci(&p);
    assert!(!any(&warnings, "linter"), "{warnings:?}");
}

#[test]
fn unknown_branching_model_is_an_error() {
    let p = Project::new();
    repo(&p, r#"{"branching": "gitflow"}"#, &[], &[]);
    let (errors, _) = check_ci(&p);
    assert!(any(&errors, "gitflow"), "{errors:?}");
}

#[test]
fn missing_pr_ci_into_integration_branch() {
    let p = Project::new();
    let wf = "on:\n  pull_request:\n    branches: [main]\njobs:\n  t:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo test\n";
    repo(&p, "{}", &[("ci.yml", wf)], &[]);
    let (errors, _) = check_ci(&p);
    assert!(any(&errors, "pull requests into 'dev'"), "{errors:?}");
    assert!(!any(&errors, "pull requests into 'main'"), "{errors:?}");
}

#[test]
fn pull_request_without_branch_filter_covers_all() {
    let p = Project::new();
    let wf = "on: [pull_request]\njobs:\n  t:\n    runs-on: ubuntu-latest\n    steps:\n      - run: pytest\n";
    repo(&p, "{}", &[("ci.yml", wf)], &[]);
    let (errors, _) = check_ci(&p);
    assert!(!any(&errors, "pull requests into"), "{errors:?}");
}

#[test]
fn release_without_prerelease_and_permissions() {
    let p = Project::new();
    let rel = "on:\n  push:\n    tags: ['v*']\njobs:\n  r:\n    runs-on: ubuntu-latest\n    steps:\n\
               \x20     - uses: softprops/action-gh-release@v2\n        with:\n          generate_release_notes: true\n";
    repo(&p, "{}", &[("release.yml", rel)], &[]);
    let (errors, _) = check_ci(&p);
    assert!(any(&errors, "contents: write"), "{errors:?}");
    assert!(any(&errors, "must publish pre-releases"), "{errors:?}");
    repo(
        &p,
        r#"{"release_candidates": false}"#,
        &[("release.yml", rel)],
        &[],
    );
    let (errors, _) = check_ci(&p);
    assert!(!any(&errors, "pre-releases"), "{errors:?}");
}

#[test]
fn prerelease_flag_computed_in_env_is_accepted() {
    let p = Project::new();
    let rel = "on:
  push:
    tags: ['v*']
permissions:
  contents: write
jobs:
  r:
    runs-on: ubuntu-latest
    env:
      IS_RC: ${{ contains(github.ref_name, '-rc') }}
    steps:
      - uses: softprops/action-gh-release@v2
        with:
          prerelease: ${{ env.IS_RC == 'true' }}
";
    repo(&p, "{}", &[("release.yml", rel)], &[]);
    let (errors, _) = check_ci(&p);
    assert!(!any(&errors, "pre-releases"), "{errors:?}");
}

#[test]
fn tag_pushed_with_github_token_cannot_trigger_release() {
    let p = Project::new();
    let tagger = "on:\n  push:\n    branches: [main]\npermissions:\n  contents: write\njobs:\n  t:\n    runs-on: ubuntu-latest\n\
                  \x20   steps:\n      - run: |\n          git tag -a v1 -m v1\n          git push origin v1\n";
    repo(&p, "{}", &[("tag.yml", tagger)], &[]);
    let (errors, _) = check_ci(&p);
    assert!(
        any(&errors, "does not trigger the release workflow"),
        "{errors:?}"
    );
    let with_pat = tagger.replace(
        "git push origin v1",
        "git push https://x:${{ secrets.RELEASE_TOKEN }}@github.com/o/r v1",
    );
    repo(&p, "{}", &[("tag.yml", &with_pat)], &[]);
    let (errors, _) = check_ci(&p);
    assert!(!any(&errors, "does not trigger"), "{errors:?}");
}

#[test]
fn missing_version_file_and_no_workflows() {
    let p = Project::new();
    p.write(
        ".dev-cycle.json",
        r#"{"devops": {"version_files": ["Cargo.toml"]}}"#,
    );
    let (errors, _) = check_ci(&p);
    assert!(any(&errors, "'Cargo.toml' does not exist"), "{errors:?}");
    assert!(any(&errors, "no GitHub Actions workflows"), "{errors:?}");
}

// --- release_status ------------------------------------------------------------------

#[test]
fn read_version_from_supported_files() {
    let p = Project::new();
    let files = [
        (
            "Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"1.2.3\"\n",
        ),
        (
            "ws/Cargo.toml",
            "[workspace]\nmembers = []\n[workspace.package]\nversion = \"1.2.3\"\n",
        ),
        ("root/Cargo.toml", "[workspace]\nmembers = [\"a\"]\n"),
        ("package.json", r#"{"name": "a", "version": "1.2.3"}"#),
        (
            "pyproject.toml",
            "[project]\nname = \"a\"\nversion = \"1.2.3\"\n",
        ),
        ("version.txt", "v1.2.3\n"),
        (
            ".claude-plugin/plugin.json",
            "{\n  \"name\": \"a\",\n  \"version\": \"1.2.3\"\n}\n",
        ),
        (
            "composer.json",
            r#"{"name": "o/a", "require": {"php": ">=8"}, "version": "1.2.3"}"#,
        ),
        (
            "manifest.json",
            r#"{"name": "a", "details": {"version": "9.9.9"}}"#,
        ),
    ];
    for (rel, text) in files {
        p.write(rel, text);
    }
    for (rel, _) in files {
        let expected = if matches!(rel, "root/Cargo.toml" | "manifest.json") {
            None
        } else {
            Some("1.2.3".to_string())
        };
        let path = p.path(rel);
        let got = read_version_file(path.to_str().unwrap()).unwrap_or_else(|_| panic!("{rel}"));
        assert_eq!(got, expected, "{rel}");
    }
}

#[test]
fn project_version_consistency() {
    let p = Project::new();
    repo(
        &p,
        r#"{"version_files": ["version.txt", "Cargo.toml"]}"#,
        &[],
        &[
            ("version.txt", "1.2.3"),
            ("Cargo.toml", "[package]\nversion = \"1.2.4\"\n"),
        ],
    );
    let root = p.root();
    let version = |root: &str| {
        let cfg = load_project_config(Some(root)).unwrap_or_else(|_| panic!("config"));
        project_version(&cfg).unwrap_or_else(|_| panic!("version"))
    };
    let (v, problems) = version(&root);
    assert!(v.is_none() && any(&problems, "disagree"), "{problems:?}");
    p.write("Cargo.toml", "[package]\nversion = \"1.2.3\"\n");
    let (v, problems) = version(&root);
    assert_eq!(v.as_deref(), Some("1.2.3"));
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn next_rc_tags() {
    let tags = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let next = |v: &str, t: &[&str]| next_rc_tag(v, &tags(t)).unwrap_or_else(|_| panic!("rc"));
    assert_eq!(
        next("1.4.0", &["v1.3.0", "v1.3.1-rc.1"]),
        (Some("v1.4.0-rc.1".into()), None)
    );
    assert_eq!(
        next("1.4.0", &["v1.4.0-rc.1", "v1.4.0-rc.2", "v1.3.9-rc.7"]),
        (Some("v1.4.0-rc.3".into()), None)
    );
    let (tag, problem) = next("1.4.0", &["v1.4.0-rc.1", "v1.4.0"]);
    assert!(tag.is_none() && problem.unwrap().contains("already released"));
}

#[test]
fn next_rc_cli_with_git_tags() {
    let p = Project::new();
    repo(
        &p,
        r#"{"version_files": ["version.txt"]}"#,
        &[],
        &[("version.txt", "2.0.0")],
    );
    p.git(&["init", "-q"]);
    p.git(&["commit", "-q", "--allow-empty", "-m", "init"]);
    p.git(&["tag", "v2.0.0-rc.1"]);
    let root = p.root();
    let res = p.run(
        "devops-release",
        "release_status",
        &["next-rc", "--dir", &root],
    );
    assert!(res.ok(), "{}", res.all());
    assert_eq!(res.all(), "v2.0.0-rc.2");
}

#[test]
fn evaluate_releases() {
    let eval = |tag: &str, data: &str, patterns: &[&str]| {
        let data = loads(data).unwrap_or_else(|_| panic!("JSON"));
        let patterns: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
        evaluate_release(tag, &data, &patterns).unwrap_or_else(|_| panic!("evaluate"))
    };
    let assets = r#"[{"name": "app-linux.tar.gz"}, {"name": "app-windows.zip"}]"#;
    assert!(
        eval(
            "v1.0.0",
            &format!(r#"{{"isPrerelease": false, "assets": {assets}}}"#),
            &["*.zip"]
        )
        .is_empty()
    );
    assert!(
        eval(
            "v1.0.0-rc.1",
            &format!(r#"{{"isPrerelease": true, "assets": {assets}}}"#),
            &[]
        )
        .is_empty()
    );
    let problems = eval(
        "v1.0.0-rc.1",
        r#"{"isPrerelease": false, "isDraft": true, "assets": []}"#,
        &["*.dmg"],
    );
    let text = problems.join("\n");
    has(
        &text,
        &["draft", "should be a pre-release", "no assets", "*.dmg"],
    );
    let problems = eval(
        "v1.0.0",
        &format!(r#"{{"isPrerelease": true, "assets": {assets}}}"#),
        &[],
    );
    assert!(any(&problems, "not a pre-release"), "{problems:?}");
}

// --- branching -----------------------------------------------------------------------

#[test]
fn cli_show_and_rc_branch() {
    let p = Project::new();
    p.write(
        ".dev-cycle.json",
        r#"{"devops": {"branching": "release-branches", "version_files": ["version.txt"]}}"#,
    );
    p.write("version.txt", "1.4.0\n");
    let root = p.root();
    let run = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend(["--dir", root.as_str()]);
        p.run("devops-release", "branching", &all)
    };
    let res = run(&["show"]);
    assert!(res.ok());
    has(&res.all(), &["release-branches", "promote workflow"]);
    let res = run(&["rc-branch"]);
    assert!(res.ok());
    assert_eq!(res.all(), "release/1.4");
    assert_eq!(
        run(&["rc-branch", "--version", "2.0.0"]).all(),
        "release/2.0"
    );
    let show = loads(&run(&["show", "--json"]).all()).unwrap_or_else(|_| panic!("JSON"));
    assert_eq!(
        show.as_dict()
            .and_then(|d| d.get("gate"))
            .and_then(|g| g.as_str()),
        Some("promote")
    );
}
