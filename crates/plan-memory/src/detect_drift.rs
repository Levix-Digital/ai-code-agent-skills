//! `detect_drift.py`: checks a proposed design, task or change against the baseline plan
//! (docs/plans) for architectural drift, scope creep or violations of the project's rules.

use kernel::out::{err, println};
use kernel::py::argparse::Parser;
use kernel::py::json::{self, LoadError};
use kernel::py::path::{abspath, dirname, isfile, join};
use kernel::py::unicode::{is_word, lower};
use kernel::py::{Crash, Dict, Value, read_text};
use vector_store::{MetaValue, Where};

use crate::index::{store_crash, sync_index};
use crate::store::PlanStore;

/// Bilingual (pt/en) because plans and proposals may be written in either language.
const DEFAULT_NEGATIVE_MARKERS: [&str; 14] = [
    "purga",
    "substituir",
    "eliminar",
    "sem",
    "remover",
    "proibido",
    "sem framework",
    "purge",
    "replace",
    "eliminate",
    "without",
    "remove",
    "forbidden",
    "no framework",
];
const DEFAULT_ALLOW_TEMPLATES: [&str; 15] = [
    "sem {tech}",
    "purga do {tech}",
    "purga de {tech}",
    "substituir {tech}",
    "substituindo {tech}",
    "substituindo o {tech}",
    "remover {tech}",
    "eliminando {tech}",
    "eliminando o {tech}",
    "without {tech}",
    "purge {tech}",
    "replace {tech}",
    "replacing {tech}",
    "remove {tech}",
    "removing {tech}",
];

pub fn main(argv: &[String]) -> u8 {
    match run(argv) {
        Ok(code) => code,
        Err(c) => c.report(),
    }
}

fn run(argv: &[String]) -> Result<u8, Crash> {
    let parser = Parser::new(
        "detect_drift.py",
        "Detect architectural drift against baseline plans in ChromaDB",
    )
    .option("--project", "Project name (default: from the repository)")
    .option(
        "--dir",
        "Project directory (default: current); plans and .dev-cycle.json are read from its repository",
    )
    .option("--id", "Specific Plan ID to validate against")
    .option("--proposal", "Inline text of proposed task/change")
    .option(
        "--file",
        "Path to markdown/text file containing the proposal",
    )
    .flag(
        "--strict",
        "Exit with non-zero code if conflicts or low score",
    );
    let args = match parser.parse(argv) {
        Ok(a) => a,
        Err(exit) => return Ok(exit.report()),
    };
    let proposal = match (args.value("file"), args.value("proposal")) {
        (Some(f), _) if !f.is_empty() => read_text(std::path::Path::new(f))?,
        (_, Some(p)) if !p.is_empty() => p.to_string(),
        _ => {
            err("❌ Error: You must supply either --proposal <text> or --file <path>\n");
            return Ok(1);
        }
    };
    let store = PlanStore::new(args.value("dir"), args.value("project"))?;
    detect_drift(
        &store,
        &proposal,
        args.value("id"),
        args.flag("strict"),
        args.value("dir"),
    )
}

/// `load_drift_rules(start_dir)`: `drift_rules` from the nearest .dev-cycle.json, walking up.
fn load_drift_rules(start_dir: &str) -> Result<(Value, Option<String>), Crash> {
    let mut current = abspath(start_dir).to_string_lossy().into_owned();
    loop {
        let manifest = join(&current, ".dev-cycle.json");
        if isfile(&manifest) {
            let reason = match json::load_file(std::path::Path::new(&manifest)) {
                Ok(Value::Dict(d)) => {
                    return Ok((
                        d.get("drift_rules")
                            .cloned()
                            .unwrap_or(Value::List(Vec::new())),
                        Some(manifest),
                    ));
                }
                Ok(other) => format!("'{}' object has no attribute 'get'", other.type_name()),
                Err(LoadError::Decode(e)) => e.to_string(),
                Err(LoadError::Crash(c)) => match c.0.split_once(": ") {
                    Some((_, message)) => message.to_string(),
                    None => c.0,
                },
            };
            err(&format!("⚠️ Could not read '{manifest}': {reason}\n"));
            return Ok((Value::List(Vec::new()), Some(manifest)));
        }
        let parent = dirname(&current);
        if parent == current {
            return Ok((Value::List(Vec::new()), None));
        }
        current = parent;
    }
}

/// `str.lower()` of a text value.
fn lower_of(v: &Value) -> Result<String, Crash> {
    match v {
        Value::Str(s) => Ok(lower(s)),
        other => Err(Crash::new(format!(
            "AttributeError: '{}' object has no attribute 'lower'",
            other.type_name()
        ))),
    }
}

/// `re.findall(r"\w+", text)`.
fn words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (is_word(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push(&text[s..i]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push(&text[s..]);
    }
    out
}

/// `re.search(r"\b" + re.escape(word) + r"\b", text)` for a run of word characters.
fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back().is_none_or(|c| !is_word(c));
        let after = text[i + word.len()..]
            .chars()
            .next()
            .is_none_or(|c| !is_word(c));
        before && after
    })
}

/// `[m.lower() for m in value]`.
fn lowered(value: &Value) -> Result<Vec<String>, Crash> {
    kernel::py::iterate(value)?.iter().map(lower_of).collect()
}

/// `rule[key]` / `rule.get(key)` of a drift rule.
fn rule_dict(rule: &Value) -> Result<&Dict, Crash> {
    match rule {
        Value::Dict(d) => Ok(d),
        other => Err(Crash::new(format!(
            "TypeError: {} indices must be integers or slices, not str",
            other.type_name()
        ))),
    }
}

pub fn detect_drift(
    store: &PlanStore,
    proposal_text: &str,
    plan_id: Option<&str>,
    strict: bool,
    project_dir: Option<&str>,
) -> Result<u8, Crash> {
    let start = project_dir.filter(|d| !d.is_empty()).unwrap_or(&store.root);
    let (tech_rules, rules_source) = load_drift_rules(start)?;

    let plans = store.load_all()?;
    let plan_id = plan_id.filter(|p| !p.is_empty());
    let found = match plan_id {
        Some(p) => store.get(p)?,
        None => PlanStore::latest_of(plans.clone()),
    };
    let Some(plan) = found else {
        let target = match plan_id {
            Some(p) => format!("plan_id='{p}'"),
            None => format!("project='{}'", store.project),
        };
        err(&format!(
            "❌ Error: Active baseline plan not found for {target} in {}\n",
            store.plans_dir
        ));
        return Ok(1);
    };

    let meta = &plan.meta;
    let pid_value = meta
        .get("plan_id")
        .cloned()
        .ok_or_else(|| Crash::new("KeyError: 'plan_id'"))?;
    let pid = pid_value.py_str();
    let baseline_doc = store.full_text(&plan)?;
    let project_name = match meta.get("project") {
        Some(p) => p.py_str(),
        None => store.project.clone(),
    };
    let plan_title = meta.get("title").unwrap_or(&pid_value).py_str();
    let mut all_milestones = Vec::new();
    for m in plan.milestones() {
        let Value::Dict(d) = &m else { continue };
        all_milestones.push(crate::store::name_of(d)?);
    }

    // 1. Semantic similarity via the local index (synced from the plan files).
    let (mut client, col) = sync_index(store, Some(plans.clone()), false, true)?;
    let results = client
        .query(
            &col,
            proposal_text,
            1,
            Some(&Where::eq("plan_id", MetaValue::Str(pid.clone()))),
        )
        .map_err(store_crash)?;
    let vector_similarity = match results.distances.first() {
        Some(d) => 1.0 / (1.0 + (*d as f64).max(0.0)),
        None => 0.5,
    };

    // 2. Milestone coverage.
    let mut addressed: Vec<Value> = Vec::new();
    let mut unaddressed: Vec<Value> = Vec::new();
    let proposal_lower = lower(proposal_text);
    for m in &all_milestones {
        let m_lower = lower_of(m)?;
        if proposal_lower.contains(&m_lower) {
            addressed.push(m.clone());
            continue;
        }
        let tokens = words(&m_lower);
        if !tokens.is_empty() && tokens.iter().all(|t| has_word(&proposal_lower, t)) {
            addressed.push(m.clone());
        } else {
            unaddressed.push(m.clone());
        }
    }

    // 3. Conflicts with the project's rules, checked against every plan still in force.
    let mut warnings: Vec<String> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();
    let mut texts = vec![baseline_doc];
    for p in &plans {
        let status = p.meta.get_or_none("status");
        let in_force =
            status.py_eq(&Value::str("active")) || status.py_eq(&Value::str("in_progress"));
        if !in_force {
            continue;
        }
        let other = p
            .meta
            .get("plan_id")
            .ok_or_else(|| Crash::new("KeyError: 'plan_id'"))?;
        if other.py_eq(&pid_value) {
            continue;
        }
        texts.push(store.full_text(p)?);
    }
    let baseline_lower = lower(&texts.join("\n"));

    let rules = kernel::py::iterate(&tech_rules)?;
    for rule in &rules {
        let d = rule_dict(rule)?;
        let tech_value = d
            .get("tech")
            .ok_or_else(|| Crash::new("KeyError: 'tech'"))?;
        let tech = lower_of(tech_value)?;
        let markers = match d.get("negative_markers") {
            Some(v) => lowered(v)?,
            None => DEFAULT_NEGATIVE_MARKERS.iter().map(|m| lower(m)).collect(),
        };
        let allow = match d.get("proposal_allow") {
            Some(v) => lowered(v)?,
            None => DEFAULT_ALLOW_TEMPLATES
                .iter()
                .map(|t| lower(&t.replace("{tech}", &tech)))
                .collect(),
        };
        let rejected = baseline_lower.contains(&tech)
            && markers.iter().any(|m| baseline_lower.contains(m.as_str()));
        if rejected
            && proposal_lower.contains(&tech)
            && !allow.iter().any(|a| proposal_lower.contains(a.as_str()))
        {
            conflicts.push(match d.get("conflict_msg") {
                Some(msg) => msg.py_str(),
                None => format!(
                    "Proposal mentions '{}', which the baseline plan rejects.",
                    tech_value.py_str()
                ),
            });
        }
    }

    // Order violation: a later milestone addressed before the one before it.
    let completed = plan.done_milestones()?;
    for (idx, m) in all_milestones.iter().enumerate() {
        if idx > 0 && addressed.iter().any(|a| a.py_eq(m)) {
            let prev = &all_milestones[idx - 1];
            if !completed.iter().any(|c| c.py_eq(prev)) && !addressed.iter().any(|a| a.py_eq(prev))
            {
                warnings.push(format!(
                    "Out-of-order execution risk: Milestone '{}' is addressed before preceding milestone '{}' is completed.",
                    m.py_str(),
                    prev.py_str()
                ));
            }
        }
    }

    // 4. Score.
    let base_score = (vector_similarity * 60.0) as i64;
    let milestone_bonus = if addressed.is_empty() { 0 } else { 30 };
    let score = (base_score + milestone_bonus + 10
        - conflicts.len() as i64 * 35
        - warnings.len() as i64 * 15)
        .clamp(5, 100);

    let (status_flag, verdict) = if !conflicts.is_empty() {
        ("🚨 DRIFT CONFLICT DETECTED", "REJECT / REALIGN")
    } else if score < 50 || warnings.len() >= 2 {
        ("⚠️ POTENTIAL DRIFT", "CAUTION / REVIEW")
    } else {
        ("🟢 ALIGNED WITH MASTER PLAN", "APPROVED FOR IMPLEMENTATION")
    };

    let rule = "=".repeat(85);
    println(&rule);
    println(&format!(
        "🛡️ PLAN DRIFT DETECTOR: [{project_name}] {plan_title}"
    ));
    println(&format!(
        "   • Baseline Plan ID : {pid} (Version: {})",
        meta.get("version").map_or("-".to_string(), Value::py_str)
    ));
    println(&format!("   • Drift Status     : {status_flag}"));
    println(&format!(
        "   • Alignment Score  : {score}% (Similarity: {vector_similarity:.2})"
    ));
    println(&format!("   • Verdict          : {verdict}"));
    if tech_rules.truthy() {
        println(&format!(
            "   • Drift Rules      : {} from {}",
            rules.len(),
            rules_source.unwrap_or_else(|| "None".into())
        ));
    } else {
        println(
            "   • Drift Rules      : none (add `drift_rules` to the project's .dev-cycle.json)",
        );
    }
    println(&"-".repeat(85));

    println("🎯 MILESTONE ALIGNMENT:");
    if addressed.is_empty() {
        println(
            "   ⚠️ No specific milestones from the baseline plan were detected in this proposal.",
        );
    } else {
        for m in &addressed {
            println(&format!("   ✅ Addressed: {}", m.py_str()));
        }
    }
    if !unaddressed.is_empty() {
        let names: Vec<String> = unaddressed.iter().map(Value::py_str).collect();
        println(&format!(
            "   ⏳ Unaddressed milestones: {}",
            names.join(", ")
        ));
    }
    if !warnings.is_empty() || !conflicts.is_empty() {
        println(&"-".repeat(85));
        println("⚠️ DETECTED WARNINGS & CONFLICTS:");
        for c in &conflicts {
            println(&format!("   🚨 CONFLICT: {c}"));
        }
        for w in &warnings {
            println(&format!("   ⚠️ WARNING : {w}"));
        }
    }
    println(&format!("{rule}\n"));

    Ok(u8::from(strict && (!conflicts.is_empty() || score < 40)))
}
