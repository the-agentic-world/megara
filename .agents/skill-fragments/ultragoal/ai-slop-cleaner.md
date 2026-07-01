# Ultragoal Fragment: AI Slop Cleaner

Use this internal fragment as a read-only completion gate over changed files.

Do not edit files. Detect and report:

- fallback-like code that masks primary failures
- duplication
- dead code
- needless abstraction
- boundary violations
- UI/design slop
- missing tests

Report:

```text
AI SLOP CLEANUP REPORT
======================

Scope: [changed files inspected]
Mode: read-only detector/report; no edits performed
Blocking Findings: [none or findings]
Advisory Findings: [none or findings]
Fallback Findings: [none or findings]
UI/Design Findings: [none/N/A or findings]
Missing Test Findings: [none or findings]
Changed Files Reviewed:
- [path] - [reviewed / no relevant edits]

Gate Result: PASS | BLOCKED
Leader Action:
- PASS: continue verification and review.
- BLOCKED: fix blocking findings only, then rerun this sweep.
Remaining Risks:
- [none or deferred risks]
```
