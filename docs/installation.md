# Installation Options

The install scripts link every skill folder of this repository into each agent's skills folder.

## Other Targets and Options

```powershell
.\install.ps1 -Targets "$HOME\.copilot\skills"              # extra agent
.\install.ps1 -Source "$HOME\.ai-code-agent-skills-personal" # link skills from another folder
.\install.ps1 -Exclude folder-sync                           # skip a skill
```
```bash
./install.sh ~/.copilot/skills
SOURCE=~/.ai-code-agent-skills-personal ./install.sh
EXCLUDE="folder-sync" ./install.sh
```

## Safety

The scripts are idempotent and safe:
- an existing link to the same skill is kept (`=`);
- a new link is created (`+`);
- an existing entry that is not a link to the source is reported and **never overwritten** (`!`);
- a target folder that is itself a link is skipped (replace it with a real folder to link skills individually).
