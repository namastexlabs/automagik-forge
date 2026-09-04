# Decisions and current work

<!-- Written by `knos export`. Commit this file. -->

A second clone reads this on its first question — it is one of the decision
records knos looks for. Nothing here is private: secrets and private paths
never reach it.


## Decisions

- **live data over static files** — MCP queries the filesystem in real time. Static files go stale; MCP does not.  _(AGENTS.md, Collectives Architecture)_
- **code is the single source of truth** — `agent-resolver.ts` is the truth, not documentation.  _(AGENTS.md, Collectives Architecture)_
- **agents are auto-discovered** — New agents are found without registry updates; no registry file to keep in sync.  _(AGENTS.md, Collectives Architecture)_
- **load only what is needed** — Spell and context loading is on demand rather than up front, to stay token efficient.  _(AGENTS.md, Spell Loading Protocol)_
- **discuss significant changes first** — Open an issue describing the problem and proposed solution, wait for maintainer feedback, get approval before implementing.  _(CONTRIBUTING.md)_
- **security issues are never public** — Report to security@namastex.ai; do not open a public issue.  _(CONTRIBUTING.md)_

## Being worked on right now

_Nothing claimed._

---
<sub>knos export. Claims lapse after 30 minutes.</sub>
