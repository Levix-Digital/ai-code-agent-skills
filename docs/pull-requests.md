# Pull Request Review on the Platform

Once a change is pushed, the pull request (a merge request on GitLab) is where it is reviewed, discussed and corrected, as in a team. The agent reads what happens there, review threads, suggestions, reviews and CI, instead of waiting for it in the chat, and guides you through each step.

## Setup

The agent works through the platform's own CLI, which keeps your login:

| Platform | CLI | Log in |
|---|---|---|
| GitHub | [`gh`](https://cli.github.com/) | `gh auth login` |
| GitLab (gitlab.com or self-hosted) | [`glab`](https://gitlab.com/gitlab-org/cli) | `glab auth login --hostname <host>` |

The platform is detected from the `origin` remote. Set it in `.dev-cycle.json` when the host name does not say it:

```json
"review": {
  "provider": "auto",
  "host": null,
  "marker": "[review]",
  "reply_marker": "[reply]",
  "auto_review": true,
  "allowed_terms": []
}
```

`provider` is `auto`, `github`, `gitlab` or `none` (review stays in the chat). Bitbucket, Azure DevOps and Gitea are planned (backlog FEAT-001).

## What Happens on a Pull Request

1. The agent opens the pull request into the work base of the branching model (`dev`, or `main` under `trunk` and `release-branches`; `release/X.Y` for a fix of a release).
2. The protocol reviews it right away, and again after pushes that change code: line comments that start with **[review]** and a level, `blocking`, `suggestion` or `nit`. The review never approves.
3. You and your teammates review as usual on the platform.
4. The agent handles every waiting thread:

   | Thread | What the agent does |
   |---|---|
   | Nit, `suggestion` block, small local ask | Fixes it, pushes, replies "**[reply]** Fixed in `<sha>`" and resolves the thread |
   | Large: design, interface or data change, several files, unclear intent | Brings it to you with the context and options; replies only after you decide; a deferred ask becomes a backlog item |
   | Question | Answers from the code and the plan |
   | Red CI or a merge conflict | Reads the logs, reproduces, fixes and pushes |

5. After changes someone requested, the agent asks them to review again.
6. Before asking you to merge, the agent runs the merge gate (`pr_sync gate-merge`): it fails while a **[review]** `blocking` thread is open, someone requested changes, a check is red or still running, or there is a conflict. It also warns when the platform does not require resolved conversations before merging; turn that on in the branch protection (GitHub) or the merge request settings (GitLab). People approve; you merge.

   The agent talks to GitHub through its REST API, which works the same on your machine, GitHub Enterprise and cloud agent environments. Only one thing needs GitHub's GraphQL API: knowing whether a review thread is resolved, and resolving it. Some environments refuse GraphQL; there the agent still reads and answers everything, tells you the resolution could not be read, and the gate keeps a **[review]** `blocking` thread holding the merge until you (or another GitHub tool the agent has) resolve it on the pull request page. GitLab needs nothing extra.

Everything the agent posts carries a marker, so its findings and answers are told apart from people's comments, and nothing it posts or commits mentions an AI (protocol principle 9; `pr_sync` refuses such text, text posted through other tools is checked with `_no_ai` (engine) and read back after posting to remove any footer the tool appended, and gate 8 checks the commit messages and the Git identities of the release). If a project's subject legitimately needs a phrase the guard flags, add it to `review.allowed_terms`.

## When the Agent Reads the Pull Request

The platform does not tell a coding agent in a terminal that someone commented. So the agent looks at fixed moments:

- at the start of every session: `/dev-cycle` lists the open pull requests that have something pending, right after a plan in progress;
- after every push, until CI finishes;
- when you say you reviewed or commented;
- on a timer, in agents that can schedule a recurring check.

Agents that receive pull request events (for example a cloud session subscribed to the pull request) react as the events arrive.

## Commands

```bash
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol pr_sync status            # what is pending on the current branch's pull request
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol pr_sync open              # open pull requests with what is pending
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol pr_sync gate-merge        # exit 1 while something holds the merge
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol pr_sync reply --thread <id> --body "Fixed in abc123."
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol pr_sync resolve --thread <id>
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol pr_sync review --file review.json
~/.ai-code-agent-skills/dev-cycle-protocol/bin/dev-cycle dev-cycle-protocol pr_sync request-review --reviewer <login>
```
