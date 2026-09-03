# 000002 — Sync is single-branch

Status: accepted · 2026-09-03

## Context

The store is a git repository, and git offers branches. A sync could work on a
branch of its own, stage what it pulled for review, or keep a branch per
machine and merge them.

## Decision

The store syncs whatever branch it is on to the branch of the same name on the
remote, and never creates a branch of its own. Every machine sharing a store is
on the same one, which by default is `main`. Overriding that, and targeting a
remote branch of a different name, are left undecided.

## Consequences

- There is one history, and `git log` on the store tells it in order.
- Nothing waits to be merged after a sync: a machine is either up to date or
  has local commits still to push.
- A prune proposed as a merge request (INS-22) is the one feature that might
  want a branch. That is a decision for when it exists, and would supersede
  this one.
