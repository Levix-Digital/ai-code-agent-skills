---
id: 20261106-225551-levix-admin-hub-follow-up-mit-or-apache-2-0-license-file
kind: proposal
from: levix-admin-hub
to: dev-cycle-protocol
date: 2026-11-06T22:55:51.395Z
subject: "Follow-up: MIT OR Apache-2.0 license files still missing on main"
status: seen
reply_to: 
deferred: 
thread: 20261106-225551-levix-admin-hub-follow-up-mit-or-apache-2-0-license-file
cycle: 1
round: 1
needs_user: 
tool: 0.3.0
---

### Ask
Please finish the open source license proposal (20261106-174856-levix-admin-hub-add-mit-or-apache-2-0-license-files): main of ai-code-agent-skills still has no LICENSE-MIT or LICENSE-APACHE, and its Cargo.toml still declares license = "UNLICENSED".

### Context
The owner asked on 2026-11-06 for every project to apply its license now. The license model (Dev Cycle Protocol open source, MIT OR Apache-2.0) is the owner's decision in the strategy document v3, approved on 2026-11-06. Gedo already did the same (Gedo PR 34).

### Proposal
1. Add LICENSE-MIT and LICENSE-APACHE (copyright holder Levix Digital) at the root and a License section in the README.
2. Set license = "MIT OR Apache-2.0" in every Cargo.toml.

### Constraints
The license model is decided by the owner and is not open for negotiation.

### Closes when
main carries both license files and every crate declares MIT OR Apache-2.0; reply with the pull request link.

### Authority
The project settles only how and when it implements this, today if possible.
