# Specs and tickets

Voon keeps its specs and tickets here as Markdown files, not as GitHub
issues. One file per spec (`NNNN-short-name.md`, numbered in order). Tickets
broken out of a spec live in a folder with the same name, under `tickets/`,
one file per ticket (`NN-slug.md`), with an index `README.md` listing each
ticket's blockers and status.

```
docs/specs/
  0001-phase-1.md                  the spec
  0001-phase-1/tickets/README.md   ticket index and frontier
  0001-phase-1/tickets/01-....md   one file per ticket
```

Every file starts with YAML front matter carrying its status. The status
replaces what an issue label would say:

```yaml
---
status: ready-for-agent
---
```

| Status | Meaning |
|---|---|
| `draft` | Still being written or discussed. |
| `ready-for-agent` | Agreed; an agent may pick it up and implement it. |
| `in-progress` | Being implemented. |
| `done` | Implemented and verified. |
| `superseded` | Replaced by a later spec (link it). |

Each ticket also has a **Blocked by** line. The frontier is every ticket
whose blockers are all `done` (and whose external gates, if any, are met).

Change the status by editing the front matter in the same commit as the
work it describes.

## Current specifications

| Spec | Scope | Tracking |
|---|---|---|
| [0001 Phase 1](0001-phase-1.md) | Initial desktop library features | See the [ticket index](0001-phase-1/tickets/README.md) for implementation and platform acceptance. |
| [0002 Library reliability and offline browsing](0002-library-reliability-and-protection.md) | Scan integrity, tag recovery, metadata candidates, CUE details, persistent covers and database version checks | in-progress · [11 tickets](0002-library-reliability-and-protection/tickets/README.md) · [acceptance](0002-library-reliability-and-protection/acceptance.md) |

Spec 0002 starts with a fresh index. Backups, restore and legacy-library
migration are outside its implementation scope.
