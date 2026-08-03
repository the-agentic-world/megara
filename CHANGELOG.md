# Changelog

## [2.0.0] - 2026-08-04

Release preparation for Planning Core v1:

- completed deterministic planning state, approval, evidence, projection, migration, doctor, and purge paths;
- removed legacy workflow runtime, Skill, hook, Team, and Ultragoal projections while retaining explicit migration evidence and rollback safety;
- added Codex MCP and Pi host integration with explicit confirmation gates;
- added release evidence generation, immutable CI archive instructions, and the v2.0.0 release decision record.

For v2.0.0 only, the user approved a plain-text decision not to cryptographically sign the Release Decision Record or tag. The release still requires final main integration, an annotated immutable unsigned `v2.0.0` tag, and a passing tag workflow archive. Future releases retain the default signed policy.
