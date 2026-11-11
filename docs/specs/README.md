# Specs and tickets

Voon keeps its specs and tickets here as Markdown files, not as GitHub
issues. One file per spec (`NNNN-short-name.md`, numbered in order); tickets
broken out of a spec go next to it.

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

Change the status by editing the front matter in the same commit as the
work it describes.
