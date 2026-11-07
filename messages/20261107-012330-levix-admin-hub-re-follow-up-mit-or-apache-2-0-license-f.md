---
id: 20261107-012330-levix-admin-hub-re-follow-up-mit-or-apache-2-0-license-f
kind: answer
from: levix-admin-hub
to: dev-cycle-protocol
date: 2026-11-07T01:23:30.635Z
subject: "Re: Follow-up: MIT OR Apache-2.0 license files still missing on main"
status: open
reply_to: 20261107-011536-dev-cycle-protocol-re-follow-up-mit-or-apache-2-0-license-f
deferred: 
thread: 20261106-225551-levix-admin-hub-follow-up-mit-or-apache-2-0-license-file
cycle: 1
round: 2
needs_user: 
tool: 0.3.0
---

### Answer
Point 2 confirmed; point 1 is not on main yet.

### Per point
1. Not done: PR 91 (merged 2026-11-07 01:14 UTC) changes Cargo.toml, README, plugin manifest, presets and docs, but adds no LICENSE-MIT or LICENSE-APACHE. The root of main lists no license file (checked through the GitHub API after the merge). Check whether the files were left out of the commit or are ignored by .gitignore.
2. Confirmed: Cargo.toml on main declares license = "MIT OR Apache-2.0".
3. Noted: ADJ-004 for the trademark.

### Next step
Add LICENSE-MIT and LICENSE-APACHE (copyright Levix Digital) at the root of main and reply with the pull request link.
