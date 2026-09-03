# 000001 — Sync stops on a conflict rather than merging

Status: accepted · 2026-09-03

## Context

`insights sync` shares one store between machines through a git remote. When
the same insight is edited on two of them, the rebase that replays local work
onto the remote's stops on a conflict, and something has to decide what the
store looks like next.

Three properties constrain that decision. An insight file is YAML frontmatter,
a prose body, and a YAML metadata footer, so interleaving two versions of one
produces a file the parser rejects. The overwhelmingly common conflict is two
edits to the same `details` body, which is prose — merging it by line yields
text that reads as neither version. And the server reads the store
continuously, so any state the sync leaves behind is a state the server will
try to parse and serve.

That last property rules out the default: a stopped rebase leaves conflict
markers in the working tree, and the server would hand those to a caller as
the insight's content.

## Decision

A conflict stops the sync. The rebase is undone, the store is left exactly as
it was, and the operator is told which insights were edited on two machines
and how to reconcile them by hand. Nothing is merged, and nothing is
discarded.

Local work is committed before anything is pulled, so the stop cannot cost an
insight this machine wrote.

## How a sync runs

The order is fixed by what each step protects:

1. Commit this machine's insights. Everything after this can fail without
   losing them.
2. Without a remote, stop here and say how to set one.
3. If the remote already has the branch, rebase the local commits onto it.
4. A conflict in that rebase is undone and reported, and the sync stops.
5. Push.

The rebase in step 3 is the only step that rewrites the working tree, and it
either completes or is aborted before the command returns. The store is never
handed back to the server in any state between the two.

## Consequences

- The store never holds conflict markers, and stays readable and searchable
  while the conflict is outstanding.
- Neither version is lost, but neither is the conflict resolved: syncing does
  not proceed on that machine until a human acts.
- That is tolerable while syncing is a command someone runs, and will not be
  once a daemon syncs on its own (INS-20) — a stalled background sync is a
  silent one.
- Reconciling by hand means raw git today. The first successor is a
  per-insight verb that takes this machine's version, the other's, or keeps
  both under two names, and re-runs the sync (INS-23).
- The automated successor is to keep both versions, taking the remote's at the
  canonical path and writing this machine's alongside it (INS-21). It waits on
  a way to fuse two insights back together (INS-22), without which keeping
  both trades a stalled sync for a store filling with near-duplicates.
