# Philosophy: A Human Window into AI-Piloted Work

## Core Purpose

Beads Task-Issue Tracker is **a human control panel** for an AI-native issue tracker.

The Beads CLI (`br`) is designed for AI agents — they create issues, update statuses, and pilot workflows programmatically. This application gives humans the visibility and controls to observe what the AI is driving, and to step in when needed.

## Following the CLI, Not Leading It

This application reads what the CLI writes and presents it for humans. It does not define the issue format, the database schema, or the sync protocol.

When the CLI evolves, we adapt. When it adds features, we surface them. But if the CLI becomes purely machine-to-machine with no human-interpretable output, we freeze at the last meaningful version. The goal is human readability.

## Why br Is Our CLI

The original Beads philosophy was simple: issues stored as JSONL, backed by SQLite, with a lightweight CLI that writes human-readable files. No daemon, no server, no complex infrastructure.

[`br`](https://github.com/Dicklesworthstone/beads_rust) (beads_rust) froze at that classic architecture: SQLite + JSONL, no daemon, no server. It is faster, more predictable, and aligned with our core principle — **a human should be able to open a project and immediately see what's happening**.

`br` is the only supported CLI. This application drives it exclusively, so there is no client choice to configure and no feature profile to adapt.

## Design Principle: Observe and Edit

Two complementary roles:

1. **Observe** — dashboards, issue lists, change detection
2. **Edit** — create, update, comment, reassign, close

The AI pilots through the CLI, the human monitors and corrects through this app. This is a control panel, not a workspace — lightweight, fast to open, immediately useful.

---

*Laurent Chapin*
