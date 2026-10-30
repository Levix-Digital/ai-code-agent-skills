//! `_onboard.py`: the rules behind `/dev-cycle`. From the state of a repository (collected by
//! `onboard`) to the options the user is offered, in the recommended order, and the summary
//! sized to what was found. No I/O here, so every rule is unit-tested.
//!
//! The state is a dict as `onboard` builds it (every key optional): repo, config, branches,
//! baseline, plans, prs, rc, bugs, untriaged, roadmap, speckit, agent, ignored, findings.

use kernel::py::{Dict, NONE, Value};

/// Option kinds that compete with a request.
const COMPETING: [&str; 4] = ["plan", "pr", "rc", "bugs"];

/// One option: `{kind, key, title, reason}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Opt {
    pub kind: &'static str,
    pub key: String,
    pub title: String,
    pub reason: String,
}

impl Opt {
    pub fn to_value(&self) -> Value {
        let mut d = Dict::new();
        d.insert("kind", Value::str(self.kind));
        d.insert("key", Value::str(&self.key));
        d.insert("title", Value::str(&self.title));
        d.insert("reason", Value::str(&self.reason));
        Value::Dict(d)
    }
}

/// `value.get(key)`, `None` when the value is not a dict.
fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.as_dict().map_or(&NONE, |d| d.get_or_none(key))
}

/// `value or []`.
fn items(value: &Value) -> &[Value] {
    value.as_list().map_or(&[], Vec::as_slice)
}

fn text(value: &Value) -> String {
    value.py_str()
}

/// `text.split(" ")[0]`.
fn first_word(value: &Value) -> String {
    text(value)
        .split(' ')
        .next()
        .unwrap_or_default()
        .to_string()
}

fn s_if(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// A new or empty repository: no product baseline, no plans and no meaningful code.
pub fn is_new(state: &Value) -> bool {
    let baseline = get(state, "baseline");
    let has_baseline = ["vision", "tech_spec", "epics", "roadmap"]
        .iter()
        .any(|k| get(baseline, k).truthy());
    !has_baseline && !get(state, "plans").truthy() && !get(get(state, "repo"), "has_code").truthy()
}

/// Missing pieces of the initial setup, in the recommended order: (key, title, reason).
pub fn setup_items(state: &Value, new: bool) -> Vec<(&'static str, String, String)> {
    let mut out = Vec::new();
    let mut item = |key, title: String, reason: &str| out.push((key, title, reason.to_string()));
    let repo = get(state, "repo");
    let config = get(state, "config");
    let branches = get(state, "branches");
    let speckit = get(state, "speckit");
    if !get(repo, "git").truthy() {
        item(
            "git",
            "Initialize the Git repository".into(),
            "there is no Git repository",
        );
    }
    if !get(config, "exists").truthy() {
        item(
            "config",
            "Create .dev-cycle.json (proposed from what was detected, written after you confirm)"
                .into(),
            "the project has no configuration",
        );
    } else if get(config, "problems").truthy() {
        let problems: Vec<String> = items(get(config, "problems")).iter().map(text).collect();
        item("config", "Fix .dev-cycle.json".into(), &problems.join("; "));
    }
    let hidden = items(get(state, "ignored"));
    if !hidden.is_empty() {
        let mut entries: Vec<String> = hidden.iter().map(|i| text(get(i, "entry"))).collect();
        entries.sort();
        entries.dedup();
        let paths: Vec<String> = hidden.iter().map(|i| text(get(i, "path"))).collect();
        item(
            "gitignore",
            format!(
                "Let the protocol's files into Git: add {} to .gitignore",
                entries.join(", ")
            ),
            &format!("ignored now: {}", paths.join(", ")),
        );
    }
    let model = match get(branches, "model") {
        v if v.truthy() => v.clone(),
        _ => Value::str("integration"),
    };
    let detected = get(get(branches, "detected"), "model");
    // The repository follows another model than the default and nobody chose yet: ask once.
    let undecided =
        !get(branches, "decided").truthy() && detected.truthy() && !detected.py_eq(&model);
    if undecided && get(config, "exists").truthy() {
        item(
            "branching",
            format!(
                "Choose the branching model (asked once): the repository looks like '{}' ({}), the configuration defaults to '{}'",
                text(detected),
                text(get(get(branches, "detected"), "reason")),
                text(&model)
            ),
            "no decision recorded",
        );
    }
    if !undecided
        && model.py_eq(&Value::str("integration"))
        && get(repo, "commits").truthy()
        && get(branches, "integration").truthy()
        && !get(branches, "integration_exists").truthy()
    {
        let production = match get(branches, "production") {
            v if v.truthy() => text(v),
            _ => "main".to_string(),
        };
        item(
            "branch",
            format!(
                "Create the integration branch '{}' from '{production}'",
                text(get(branches, "integration"))
            ),
            "feature work goes through the integration branch",
        );
    }
    let agent = get(state, "agent");
    if get(agent, "name").py_eq(&Value::str("claude-code"))
        && !get(agent, "attribution_hidden").truthy()
    {
        item(
            "attribution",
            "Turn off the agent's automatic attribution in commits and pull requests (.claude/settings.json)".into(),
            "authorship belongs to the repository's Git identity",
        );
    }
    let state_name = get(speckit, "state");
    if state_name.py_eq(&Value::str("unmanaged")) {
        item(
            "speckit-adopt",
            "Adopt the Spec Kit already in the repository (pin its version, preset, feat/NNN-slug branches; the hand-written constitution is kept and moved into the sources)".into(),
            "Spec Kit is used here without the protocol",
        );
    } else if state_name.py_eq(&Value::str("enabled")) && get(speckit, "problems").truthy() {
        let problems: Vec<String> = items(get(speckit, "problems")).iter().map(text).collect();
        item(
            "speckit-fix",
            "Fix the Spec Kit setup".into(),
            &problems.join("; "),
        );
    } else if state_name.py_eq(&Value::str("undecided")) && !new {
        if get(speckit, "uv").truthy() {
            item(
                "speckit-ask",
                "Decide whether to use Spec Kit for the detail of features (asked once)".into(),
                "no decision recorded",
            );
        } else {
            item(
                "speckit-ask",
                "Decide whether to use Spec Kit (needs uv: https://docs.astral.sh/uv/)".into(),
                "no decision recorded; uv is not installed",
            );
        }
    }
    out
}

/// Every option that applies, in the recommended order.
pub fn options(state: &Value, request: Option<&str>) -> Vec<Opt> {
    let new = is_new(state);
    let mut out = Vec::new();
    let mut add = |kind, key: String, title: String, reason: String| {
        out.push(Opt {
            kind,
            key,
            title,
            reason,
        });
    };

    if new {
        for (key, title, reason) in setup_items(state, new) {
            if matches!(key, "git" | "config" | "branch") {
                continue; // Phase 0 step 1 sets up the repository and the configuration
            }
            add("setup", key.to_string(), title, reason);
        }
        let idea = match request {
            Some(r) => format!(": {r}"),
            None => " (tell the agent your idea)".to_string(),
        };
        add(
            "baseline",
            "inception".into(),
            format!("Start the product from the idea{idea}"),
            "new repository: Phase 0, product inception".into(),
        );
        return out;
    }

    for plan in items(get(state, "plans")) {
        let total = get(plan, "total");
        let done = get(plan, "done");
        let left = total.as_i64().unwrap_or(0) - done.as_i64().unwrap_or(0);
        let progress = if total.truthy() {
            format!(
                "{}/{} milestones",
                get_or_zero(plan, "done"),
                get_or_zero(plan, "total")
            )
        } else {
            "no milestones".to_string()
        };
        if total.truthy() && left <= 0 {
            continue;
        }
        let id = get(plan, "id");
        let title = match get(plan, "title") {
            t if t.truthy() => text(t),
            _ => text(id),
        };
        add(
            "plan",
            format!("plan:{}", text(id)),
            format!("Resume the plan '{title}' ({progress})"),
            format!("plan {}", text(get(plan, "status"))),
        );
    }
    for pr in items(get(state, "prs")) {
        if get(pr, "needs_action").truthy() {
            let for_user = get(pr, "for_user");
            add(
                "pr",
                format!("pr:{}", text(get(pr, "number"))),
                format!("Work on {}", text(get(pr, "summary"))),
                if for_user.truthy() {
                    format!("{} item(s) need your decision", text(for_user))
                } else {
                    "review on the platform".into()
                },
            );
        }
    }
    let rc = get(state, "rc");
    if rc.truthy() && !get(rc, "released").truthy() {
        let run = text(get(rc, "run"));
        let not_run = get(rc, "not_run");
        let failed = get(rc, "failed");
        if not_run.truthy() || failed.truthy() {
            let mut parts = Vec::new();
            if not_run.truthy() {
                parts.push(format!("{} not run", text(not_run)));
            }
            if failed.truthy() {
                parts.push(format!("{} failed", text(failed)));
            }
            add(
                "rc",
                format!("rc:{run}"),
                format!("Finish the acceptance of {run} ({})", parts.join(", ")),
                "release candidate pending".into(),
            );
        } else {
            let step = if get(get(state, "branches"), "gate").py_eq(&Value::str("promote")) {
                "gate 8, then you promote it (promote workflow)"
            } else {
                "open the release pull request (gate 8)"
            };
            add(
                "rc",
                format!("rc:{run}"),
                format!("{run} is accepted: {step}"),
                "release gate pending".into(),
            );
        }
    }
    let bugs = items(get(state, "bugs"));
    if !bugs.is_empty() {
        let ids: Vec<String> = bugs.iter().map(first_word).collect();
        add(
            "bugs",
            "bugs".into(),
            format!("Plan the open blocking bugs: {}", ids.join(", ")),
            "bugs come first".into(),
        );
    }
    let untriaged = items(get(state, "untriaged"));
    if !untriaged.is_empty() {
        let ids: Vec<String> = untriaged.iter().map(first_word).collect();
        add(
            "bugs",
            "triage".into(),
            format!("Triage the open bugs: {}", ids.join(", ")),
            "no known bug stays untriaged".into(),
        );
    }
    for (key, title, reason) in setup_items(state, new) {
        add("setup", key.to_string(), title, reason);
    }
    let baseline = get(state, "baseline");
    if !(get(baseline, "vision").truthy() && get(baseline, "tech_spec").truthy()) {
        add(
            "baseline",
            "adoption".into(),
            "Adopt a product baseline (Phase 0, adoption mode): vision, technical specification, backlog with epics, roadmap".into(),
            "the project has no product baseline".into(),
        );
    }
    let roadmap = get(state, "roadmap");
    if get(roadmap, "current").truthy() && get(roadmap, "open_items").truthy() {
        let ids: Vec<String> = items(get(roadmap, "open_items"))
            .iter()
            .take(5)
            .map(first_word)
            .collect();
        add(
            "release",
            "release".into(),
            format!(
                "Next items of {}: {}",
                text(get(roadmap, "current")),
                ids.join(", ")
            ),
            "current release of the roadmap".into(),
        );
    }
    if let Some(r) = request {
        add(
            "request",
            "request".into(),
            format!("Your request: {r}"),
            "Phase 1, discovery".into(),
        );
    }
    out
}

/// `str(value.get(key, 0))`.
fn get_or_zero(value: &Value, key: &str) -> String {
    value
        .as_dict()
        .and_then(|d| d.get(key))
        .map_or("0".to_string(), text)
}

fn kinds(opts: &[Opt]) -> Vec<&'static str> {
    opts.iter().map(|o| o.kind).collect()
}

fn competes(kinds: &[&str]) -> bool {
    kinds.iter().any(|k| COMPETING.contains(k))
}

/// What the agent does: `{mode, ask, recommended, options, summary}`. `ask` is false only when
/// nothing competes with the request and nothing is missing.
pub fn recommendation(state: &Value, request: Option<&str>) -> Dict {
    let new = is_new(state);
    let opts = options(state, request);
    let k = kinds(&opts);
    let (mode, ask, recommended) = if new {
        let ask = k.contains(&"setup") || request.is_none();
        let pick = if ask { opts.first() } else { opts.last() };
        ("new", ask, pick.map_or(Value::Null, |o| Value::str(&o.key)))
    } else {
        let ask =
            competes(&k) || k.contains(&"setup") || k.contains(&"baseline") || request.is_none();
        (
            "existing",
            ask,
            opts.first().map_or(Value::Null, |o| Value::str(&o.key)),
        )
    };
    let mut d = Dict::new();
    d.insert("mode", Value::str(mode));
    d.insert("ask", Value::Bool(ask));
    d.insert("recommended", recommended);
    d.insert(
        "options",
        Value::List(opts.iter().map(Opt::to_value).collect()),
    );
    d.insert("summary", Value::str(summary(state, &opts, request, new)));
    d
}

/// One line when there is nothing to decide; otherwise a short heading for the options.
pub fn summary(state: &Value, opts: &[Opt], request: Option<&str>, new: bool) -> String {
    if new {
        return "New repository: the cycle starts with Phase 0 (product inception).".into();
    }
    let roadmap = get(state, "roadmap");
    let release = if get(roadmap, "current").truthy() {
        format!("current release {}", text(get(roadmap, "current")))
    } else {
        "no roadmap".to_string()
    };
    let count = |n: usize| {
        if n == 0 {
            "no".to_string()
        } else {
            n.to_string()
        }
    };
    let bugs = items(get(state, "bugs")).len();
    let plans = opts.iter().filter(|o| o.kind == "plan").count();
    let prs = opts.iter().filter(|o| o.kind == "pr").count();
    let mut line = format!(
        "{release}, {} blocking bug{}, {} plan{} in progress",
        count(bugs),
        s_if(bugs),
        count(plans),
        s_if(plans)
    );
    if prs > 0 {
        line += &format!(", {prs} pull request{} waiting", s_if(prs));
    }
    let k = kinds(opts);
    let setup = k.contains(&"setup");
    if let Some(r) = request
        && !competes(&k)
        && !setup
        && !k.contains(&"baseline")
    {
        return format!("Project up to date ({line}). Going on with: {r}");
    }
    if setup && !competes(&k) && !get(state, "plans").truthy() {
        return format!(
            "This repository is not set up for the cycle yet ({line}). Initial setup, in the recommended order:"
        );
    }
    format!("Project state: {line}. Where to start:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::py::json::loads;

    const SET_UP: &str = r#"{"repo": {"git": true, "commits": 5, "has_code": true},
        "config": {"exists": true, "problems": []},
        "branches": {"integration": "dev", "integration_exists": true, "production": "main"},
        "baseline": {"vision": true, "tech_spec": true, "style": "layered", "tec_doc": true, "epics": 2, "roadmap": true},
        "speckit": {"state": "declined", "uv": true},
        "roadmap": {"current": "v0.2.0 Albums", "open_items": ["FEAT-004 (high) Albums"]}}"#;

    fn json(text: &str) -> Value {
        loads(text).expect("valid JSON")
    }

    /// `SET_UP` with some keys replaced.
    fn state(changes: &str) -> Value {
        let Value::Dict(mut s) = json(SET_UP) else {
            unreachable!()
        };
        if let Value::Dict(c) = json(changes) {
            for (k, v) in c.iter() {
                s.insert_value(k.clone(), v.clone());
            }
        }
        Value::Dict(s)
    }

    fn keys(opts: &[Opt]) -> Vec<String> {
        opts.iter().map(|o| o.key.clone()).collect()
    }

    fn option_kinds(rec: &Dict) -> Vec<String> {
        items(rec.get_or_none("options"))
            .iter()
            .map(|o| text(get(o, "kind")))
            .collect()
    }

    fn option(rec: &Dict, i: usize, field: &str) -> String {
        text(get(&items(rec.get_or_none("options"))[i], field))
    }

    fn field(rec: &Dict, key: &str) -> String {
        text(rec.get_or_none(key))
    }

    #[test]
    fn nothing_pending_goes_straight_on_with_one_line() {
        let rec = recommendation(&state("{}"), Some("add an export command"));
        assert_eq!(field(&rec, "mode"), "existing");
        assert!(matches!(rec.get_or_none("ask"), Value::Bool(false)));
        assert!(field(&rec, "summary").starts_with(
            "Project up to date (current release v0.2.0 Albums, no blocking bugs, no plans in progress)"
        ));
        assert!(field(&rec, "summary").ends_with("Going on with: add an export command"));
        assert_eq!(option_kinds(&rec), ["release", "request"]);
    }

    #[test]
    fn without_a_request_the_options_are_shown() {
        let rec = recommendation(&state("{}"), None);
        assert!(rec.get_or_none("ask").truthy());
        assert_eq!(field(&rec, "recommended"), "release");
    }

    #[test]
    fn competing_items_come_first_in_order_and_ask() {
        let s = state(
            r#"{"plans": [{"id": "p", "title": "Export", "status": "in_progress", "done": 1, "total": 3}],
                "rc": {"run": "v1.0.0-rc.1", "version": "v1.0.0", "not_run": 2, "failed": 1, "released": false},
                "bugs": ["BUG-007 (high) Crash on start"]}"#,
        );
        let rec = recommendation(&s, Some("add search"));
        assert!(rec.get_or_none("ask").truthy());
        assert_eq!(field(&rec, "recommended"), "plan:p");
        assert_eq!(
            option_kinds(&rec),
            ["plan", "rc", "bugs", "release", "request"]
        );
        assert!(option(&rec, 0, "title").contains("Resume the plan 'Export' (1/3 milestones)"));
        assert!(
            option(&rec, 1, "title")
                .contains("Finish the acceptance of v1.0.0-rc.1 (2 not run, 1 failed)")
        );
        assert!(option(&rec, 2, "title").contains("BUG-007"));
    }

    #[test]
    fn untriaged_bugs_compete_with_the_request() {
        let s = state(
            r#"{"untriaged": ["BUG-004 (low) Typo: not scheduled; it goes into v0.2.0 at the latest"]}"#,
        );
        let rec = recommendation(&s, Some("add search"));
        assert!(rec.get_or_none("ask").truthy());
        assert_eq!(field(&rec, "recommended"), "triage");
        assert!(option(&rec, 0, "title").contains("Triage the open bugs: BUG-004"));
    }

    #[test]
    fn finished_plans_and_released_candidates_are_not_pending() {
        let s = state(
            r#"{"plans": [{"id": "p", "title": "Done", "status": "in_progress", "done": 3, "total": 3}],
                "rc": {"run": "v1.0.0-rc.2", "version": "v1.0.0", "not_run": 0, "failed": 0, "released": true}}"#,
        );
        assert_eq!(keys(&options(&s, None)), ["release"]);
    }

    #[test]
    fn an_accepted_candidate_waits_for_the_release_gate() {
        let s = state(
            r#"{"rc": {"run": "v1.0.0-rc.2", "version": "v1.0.0", "not_run": 0, "failed": 0, "released": false}}"#,
        );
        assert!(
            options(&s, None)[0]
                .title
                .contains("is accepted: open the release pull request")
        );
    }

    #[test]
    fn never_used_repository_lists_the_initial_setup_in_order() {
        let s = state(
            r#"{"config": {"exists": false},
                "branches": {"integration": "dev", "integration_exists": false, "production": "main"},
                "baseline": {}, "speckit": {"state": "undecided", "uv": true}, "roadmap": null}"#,
        );
        let rec = recommendation(&s, Some("fix the login"));
        assert!(rec.get_or_none("ask").truthy());
        assert!(
            field(&rec, "summary").starts_with("This repository is not set up for the cycle yet")
        );
        assert_eq!(
            keys(&options(&s, Some("fix the login"))),
            ["config", "branch", "speckit-ask", "adoption", "request"]
        );
    }

    #[test]
    fn spec_kit_states() {
        assert!(!keys(&options(&state("{}"), None)).contains(&"speckit-ask".to_string()));
        let unmanaged = options(
            &state(r#"{"speckit": {"state": "unmanaged", "uv": true}}"#),
            None,
        );
        assert_eq!(unmanaged[0].key, "speckit-adopt");
        assert!(
            unmanaged[0]
                .title
                .contains("hand-written constitution is kept")
        );
        let broken = options(
            &state(
                r#"{"speckit": {"state": "enabled", "uv": true, "problems": ["preset 'dev-cycle' is not installed"]}}"#,
            ),
            None,
        );
        assert_eq!(broken[0].key, "speckit-fix");
        assert!(broken[0].reason.contains("preset"));
        let no_uv = options(
            &state(r#"{"speckit": {"state": "undecided", "uv": false}}"#),
            None,
        );
        assert_eq!(no_uv[0].key, "speckit-ask");
        assert!(no_uv[0].title.contains("needs uv"));
    }

    #[test]
    fn new_repository_starts_phase_0() {
        let s = json(
            r#"{"repo": {"git": true, "commits": 0, "has_code": false}, "config": {"exists": false},
                "baseline": {}, "speckit": {"state": "undecided", "uv": true}}"#,
        );
        assert!(is_new(&s));
        let rec = recommendation(&s, Some("an app where families keep their photos"));
        assert_eq!(field(&rec, "mode"), "new");
        assert!(matches!(rec.get_or_none("ask"), Value::Bool(false)));
        assert_eq!(field(&rec, "recommended"), "inception");
        let opts = items(rec.get_or_none("options"));
        assert_eq!(
            text(get(&opts[opts.len() - 1], "title")),
            "Start the product from the idea: an app where families keep their photos"
        );
        assert!(recommendation(&s, None).get_or_none("ask").truthy());
    }

    #[test]
    fn code_without_baseline_is_an_existing_project() {
        let s = json(
            r#"{"repo": {"git": true, "commits": 3, "has_code": true}, "config": {"exists": true, "problems": []},
                "branches": {"integration": "dev", "integration_exists": true}, "baseline": {},
                "speckit": {"state": "declined"}}"#,
        );
        assert!(!is_new(&s));
        assert_eq!(keys(&options(&s, Some("x"))), ["adoption", "request"]);
    }

    #[test]
    fn a_repository_on_another_model_is_asked_once() {
        let branches = |decided: bool, model: &str| {
            state(&format!(
                r#"{{"branches": {{"integration": "dev", "integration_exists": false, "production": "main",
                    "model": "{model}", "decided": {decided},
                    "detected": {{"model": "trunk", "integration_branch": null,
                                  "reason": "only 'main' and short-lived branches"}}}}}}"#
            ))
        };
        let opts = options(&branches(false, "integration"), None);
        assert_eq!(opts[0].key, "branching");
        assert!(!keys(&opts).contains(&"branch".to_string()));
        assert!(opts[0].title.contains("looks like 'trunk'"));
        // integration chosen: its branch is proposed
        assert_eq!(
            options(&branches(true, "integration"), None)[0].key,
            "branch"
        );
        // the default already matches: nothing to ask
        assert!(
            !keys(&options(&branches(false, "trunk"), None)).contains(&"branching".to_string())
        );
    }

    #[test]
    fn an_accepted_candidate_under_promote_names_the_promotion() {
        let s = state(
            r#"{"rc": {"run": "v1.0.0-rc.2", "version": "v1.0.0", "not_run": 0, "failed": 0, "released": false},
                "branches": {"integration": "dev", "integration_exists": true, "production": "main", "gate": "promote"}}"#,
        );
        assert!(options(&s, None)[0].title.contains("then you promote it"));
    }

    #[test]
    fn config_problems_are_a_setup_item() {
        let opts = options(
            &state(
                r#"{"config": {"exists": true, "problems": ["speckit.version 'latest' must be an exact release tag"]}}"#,
            ),
            None,
        );
        assert_eq!(opts[0].key, "config");
        assert!(opts[0].reason.contains("exact release tag"));
    }

    #[test]
    fn attribution_item_only_for_claude_code_without_settings() {
        let keys_of =
            |agent: &str| keys(&options(&state(&format!(r#"{{"agent": {agent}}}"#)), None));
        assert!(!keys(&options(&state("{}"), None)).contains(&"attribution".to_string()));
        assert!(
            !keys_of(r#"{"name": "codex", "attribution_hidden": false}"#)
                .contains(&"attribution".to_string())
        );
        assert_eq!(
            keys_of(r#"{"name": "claude-code", "attribution_hidden": false}"#)[0],
            "attribution"
        );
        assert!(
            !keys_of(r#"{"name": "claude-code", "attribution_hidden": true}"#)
                .contains(&"attribution".to_string())
        );
    }

    #[test]
    fn open_pull_requests_with_pending_items_come_right_after_plans() {
        let s = state(
            r#"{"plans": [{"id": "p", "title": "Export", "status": "in_progress", "done": 1, "total": 3}],
                "prs": [{"number": 12, "title": "Sharing", "summary": "PR #12 Sharing: 2 threads waiting (1 for you), CI red",
                         "needs_action": true, "for_user": 1},
                        {"number": 13, "title": "Docs", "summary": "PR #13 Docs: waiting for review",
                         "needs_action": false, "for_user": 0}],
                "bugs": ["BUG-1 (high) Crash"]}"#,
        );
        let rec = recommendation(&s, Some("add search"));
        let opts = options(&s, Some("add search"));
        assert_eq!(keys(&opts)[..3], ["plan:p", "pr:12", "bugs"]);
        assert_eq!(
            option(&rec, 1, "title"),
            "Work on PR #12 Sharing: 2 threads waiting (1 for you), CI red"
        );
        assert_eq!(option(&rec, 1, "reason"), "1 item(s) need your decision");
        assert!(field(&rec, "summary").contains("1 pull request waiting"));
    }

    #[test]
    fn a_pending_pull_request_stops_the_straight_path() {
        let s = state(
            r#"{"prs": [{"number": 12, "title": "x", "summary": "PR #12 x: CI red", "needs_action": true, "for_user": 0}]}"#,
        );
        assert!(
            recommendation(&s, Some("add search"))
                .get_or_none("ask")
                .truthy()
        );
    }

    #[test]
    fn the_ignored_files_are_one_item_with_sorted_entries() {
        let s = state(
            r#"{"ignored": [{"path": "TECDOC.md", "entry": "!/TECDOC.md"}, {"path": "docs/BACKLOG.md", "entry": "!/docs/"},
                            {"path": "docs/ROADMAP.md", "entry": "!/docs/"}]}"#,
        );
        let opts = options(&s, None);
        let item = opts
            .iter()
            .find(|o| o.key == "gitignore")
            .expect("gitignore item");
        assert_eq!(
            item.title,
            "Let the protocol's files into Git: add !/TECDOC.md, !/docs/ to .gitignore"
        );
        assert_eq!(
            item.reason,
            "ignored now: TECDOC.md, docs/BACKLOG.md, docs/ROADMAP.md"
        );
    }
}
