"""
Plan Drift Detector Utility - plan-memory Skill
Analyzes proposed architectural designs, tasks, or code modifications against
the active baseline plan stored in ChromaDB to detect architectural drift,
scope creep, or principle violations.
"""
import os
import sys
import argparse
import re
import chromadb

from _common import get_db_path, normalize_project

def find_active_plan(col, plan_id: str = None, project: str = None):
    if plan_id:
        res = col.get(ids=[plan_id])
        if res and res["ids"]:
            return res["ids"][0], res["documents"][0], res["metadatas"][0]
        return None, None, None
    elif project:
        res = col.get(where={"project": project})
        if res and res["ids"]:
            items = list(zip(res["ids"], res["documents"], res["metadatas"]))
            def sort_key(item):
                st = (item[2] or {}).get("status", "")
                st_prio = 0 if st in ["active", "in_progress"] else 1
                dt = (item[2] or {}).get("created_at", "")
                return (st_prio, -len(dt), dt)
            items.sort(key=sort_key)
            return items[0]
    return None, None, None

def detect_drift(
    proposal_text: str,
    project: str = None,
    plan_id: str = None,
    strict: bool = False
):
    if sys.stdout.encoding != 'utf-8':
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    db_path = get_db_path()
    if not os.path.exists(db_path):
        print("❌ Error: No plans database found.", file=sys.stderr)
        sys.exit(1)

    client = chromadb.PersistentClient(path=db_path)
    col = client.get_or_create_collection("project_plans")

    pid, baseline_doc, meta = find_active_plan(col, plan_id=plan_id, project=project)
    if not pid:
        target = f"plan_id='{plan_id}'" if plan_id else f"project='{project}'"
        print(f"❌ Error: Active baseline plan not found for {target}", file=sys.stderr)
        sys.exit(1)

    meta = meta or {}
    project_name = meta.get("project", "Unknown")
    plan_title = meta.get("title", pid)
    milestones_str = meta.get("milestones", "")
    all_milestones = [m.strip() for m in milestones_str.split(",") if m.strip()]

    # 1. Semantic Similarity via ChromaDB vector distance
    query_res = col.query(
        query_texts=[proposal_text],
        n_results=1,
        where={"plan_id": pid} if "plan_id" in meta else None
    )

    vector_similarity = 0.5
    if query_res and "distances" in query_res and query_res["distances"] and len(query_res["distances"][0]) > 0:
        dist = query_res["distances"][0][0]
        # Cosine / L2 distance to similarity
        vector_similarity = 1.0 / (1.0 + max(0.0, dist))

    # 2. Milestone Coverage Analysis
    addressed_milestones = []
    unaddressed_milestones = []
    proposal_lower = proposal_text.lower()
    for m in all_milestones:
        m_lower = m.lower()
        # Direct substring match
        if m_lower in proposal_lower:
            addressed_milestones.append(m)
            continue
        # Or all tokens matched with word boundaries
        tokens = [t for t in re.findall(r'\w+', m_lower) if len(t) > 0]
        if tokens and all(re.search(r'\b' + re.escape(t) + r'\b', proposal_lower) for t in tokens):
            addressed_milestones.append(m)
        else:
            unaddressed_milestones.append(m)

    # 3. Known Conflict & Anti-Pattern Detection
    warnings = []
    conflicts = []

    # Rules to check against baseline intentions.
    # Markers are bilingual (pt/en) because plans and proposals may be written in either language.
    tech_rules = [
        {
            "tech": "langchain",
            "negative_markers": ["purga", "substituir", "eliminar", "sem", "remover", "proibido", "sem framework",
                                 "purge", "replace", "eliminate", "without", "remove", "forbidden", "no framework"],
            "proposal_allow": ["sem langchain", "purga do langchain", "substituir langchain", "remover langchain", "eliminando langchain",
                               "without langchain", "purge langchain", "replace langchain", "remove langchain", "removing langchain"],
            "conflict_msg": "Proposal suggests introducing or using 'LangChain', but baseline plan mandates its purge/removal in favor of pure Rust native architecture."
        },
        {
            "tech": "langgraph",
            "negative_markers": ["purga", "substituir", "eliminar", "sem", "remover", "proibido",
                                 "purge", "replace", "eliminate", "without", "remove", "forbidden"],
            "proposal_allow": [
                "sem langgraph", "purga do langgraph", "substituir langgraph", 
                "remover langgraph", "eliminando langgraph", "eliminando o langgraph", 
                "substituindo o langgraph", "substituindo langgraph", "purga de langgraph",
                "without langgraph", "purge langgraph", "replace langgraph", "remove langgraph",
                "removing langgraph", "replacing langgraph"
            ],
            "conflict_msg": "Proposal suggests using 'LangGraph', but baseline plan mandates its replacement with a native Rust FSM."
        },
        {
            "tech": "banco paralelo",  # Portuguese for "parallel database"
            "negative_markers": ["single source", "single database", "settings.db", "paridade"],
            "proposal_allow": [],
            "conflict_msg": "Proposal hints at parallel databases ('banco paralelo'), violating Single Source of Truth (`settings.db`)."
        }
    ]

    for rule in tech_rules:
        tech = rule["tech"]
        # Check if baseline intends to purge / eliminate this tech
        baseline_has_rejection = any(
            marker in baseline_doc.lower() and tech in baseline_doc.lower()
            for marker in rule["negative_markers"]
        )
        if baseline_has_rejection and tech in proposal_lower:
            is_allowed = any(allow in proposal_lower for allow in rule["proposal_allow"])
            if not is_allowed:
                conflicts.append(rule["conflict_msg"])

    # Check for order violation (e.g. attempting later milestones first)
    if all_milestones:
        completed_list = [c.strip() for c in meta.get("completed_milestones", "").split(",") if c.strip()]
        for idx, m in enumerate(all_milestones):
            if m in addressed_milestones and idx > 0:
                prev_m = all_milestones[idx - 1]
                if prev_m not in completed_list and prev_m not in addressed_milestones:
                    warnings.append(f"Out-of-order execution risk: Milestone '{m}' is addressed before preceding milestone '{prev_m}' is completed.")

    # 4. Score Calculation
    # A single focused task addressing an active milestone should score high if aligned!
    base_score = int(vector_similarity * 60)
    milestone_bonus = 30 if addressed_milestones else 0
    alignment_score = max(5, min(100, base_score + milestone_bonus + 10 - (len(conflicts) * 35) - (len(warnings) * 15)))

    if conflicts:
        status_flag = "🚨 DRIFT CONFLICT DETECTED"
        verdict = "REJECT / REALIGN"
    elif alignment_score < 50 or len(warnings) >= 2:
        status_flag = "⚠️ POTENTIAL DRIFT"
        verdict = "CAUTION / REVIEW"
    else:
        status_flag = "🟢 ALIGNED WITH MASTER PLAN"
        verdict = "APPROVED FOR IMPLEMENTATION"

    # Display Report
    print("=" * 85)
    print(f"🛡️ PLAN DRIFT DETECTOR: [{project_name}] {plan_title}")
    print(f"   • Baseline Plan ID : {pid} (Version: {meta.get('version', '-')})")
    print(f"   • Drift Status     : {status_flag}")
    print(f"   • Alignment Score  : {alignment_score}% (Similarity: {vector_similarity:.2f})")
    print(f"   • Verdict          : {verdict}")
    print("-" * 85)

    print("🎯 MILESTONE ALIGNMENT:")
    if addressed_milestones:
        for m in addressed_milestones:
            print(f"   ✅ Addressed: {m}")
    else:
        print("   ⚠️ No specific milestones from the baseline plan were detected in this proposal.")

    if unaddressed_milestones:
        print(f"   ⏳ Unaddressed milestones: {', '.join(unaddressed_milestones)}")

    if warnings or conflicts:
        print("-" * 85)
        print("⚠️ DETECTED WARNINGS & CONFLICTS:")
        for c in conflicts:
            print(f"   🚨 CONFLICT: {c}")
        for w in warnings:
            print(f"   ⚠️ WARNING : {w}")

    print("=" * 85 + "\n")

    if strict and (conflicts or alignment_score < 40):
        sys.exit(1)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Detect architectural drift against baseline plans in ChromaDB")
    parser.add_argument("--project", type=str, default=None, help="Project name (e.g. AnyContext)")
    parser.add_argument("--id", type=str, default=None, help="Specific Plan ID to validate against")
    parser.add_argument("--proposal", type=str, default=None, help="Inline text of proposed task/change")
    parser.add_argument("--file", type=str, default=None, help="Path to markdown/text file containing the proposal")
    parser.add_argument("--strict", action="store_true", help="Exit with non-zero code if conflicts or low score")

    args = parser.parse_args()

    proposal_content = ""
    if args.file:
        with open(args.file, "r", encoding="utf-8") as f:
            proposal_content = f.read()
    elif args.proposal:
        proposal_content = args.proposal
    else:
        print("❌ Error: You must supply either --proposal <text> or --file <path>", file=sys.stderr)
        sys.exit(1)

    detect_drift(
        proposal_text=proposal_content,
        project=normalize_project(args.project),
        plan_id=args.id,
        strict=args.strict
    )
