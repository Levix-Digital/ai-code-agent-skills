# Manual tests

## Sync

### MT-SYNC-001: Sync a folder
- **Covers**: FR-1
- **Prerequisites**: an account
- **Steps**: open the app and sync a folder
- **Expected**: the folder is synced
- **Result** (v1.0.0-rc.1): [x] Pass [ ] Fail [ ] Blocked
- **Notes**: fine

### MT-SYNC-002: Resolve a conflict
- **Covers**: FR-2
- **Steps**: edit the same file on two machines
- **Expected**: the conflict is shown
- **Result** (v1.0.0-rc.1): [ ] Pass [x] Fail [ ] Blocked
- **Notes**: crashed on save
  the log shows a null path

```md
### not a test: inside a fence
```

## Export

### MT-EXP-001: Export CSV
- **Covers**: FR-3
- **Steps**: export the report
- **Expected**: a CSV file
- **Result** (v1.0.0-rc.1): [ ] Pass [ ] Fail [X] Blocked
- **Notes**:

### MT-EXP-002: Export PDF
- **Covers**: FR-4
- **Steps**: export as PDF
- **Expected**: a PDF file
- **Result** (v0.9.0-rc.2): [x] Pass [ ] Fail [ ] Blocked
- **Notes**: checked by hand

### MT-EXP-003: Export without a feature
- **Covers**: FR-5
- **Steps**: export again
- **Expected**: works
- **Result** (v1.0.0-rc.1): [ ] Pass [x] Fail [ ] Blocked
- **Notes**:
