# Manual Tests

Curated manual test suite: one section per feature, each test describing how the software behaves **today**. Tests are edited in place when behavior changes and removed when they no longer apply. Only what cannot be automated is tested here.

How to record a run: in each test of the current run, mark exactly one box of the `Result` line with `x` and, for `Fail` or `Blocked`, describe what happened in `Notes`.

## <Feature Name>

### MT-AREA-001: <What the user does and what must happen>

- **Covers**: <subsystem, command or screen>
- **Prerequisites**: <state, files, accounts or settings needed before starting>
- **Steps**:
  1. <Exact action or copyable command>
  2. <...>
- **Expected**:
  - <Observable result: output, screen, file, log line>
- **Result** (<run label>): [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:

### MT-AREA-002: <...>

- **Covers**: <...>
- **Prerequisites**: <...>
- **Steps**:
  1. <...>
- **Expected**:
  - <...>
- **Result**: [ ] Pass [ ] Fail [ ] Blocked
- **Notes**:
