# Interaction Model: Group Chat with Mentions

## Problem

The current system confuses all participants about who is speaking and how to reply.
Sometimes they reply normally, sometimes through MCP tools. The captain gets vague
notifications about mate activity and has to ask for updates that may be wrong.
The summarizer editorializes and fires too often.

## @Mention Routing

All communication uses mandatory @mentions. A message without a mention gets bounced
by the system with a prompt to address someone.

Mentions are **routing**: `@mate` means only the mate sees it. `@human` means only
the human sees it. The human has a feed where they can see more than their direct
mentions.

Multi-mentions (e.g. `@human @mate`) are allowed but expected to be rare.

Participants: `@human`, `@captain`, `@mate`, `@admiral`, `@summarizer`.

### What this replaces

- `captain_steer` → just `@mate here's a correction`
- `captain_notify_human` → just `@human need a decision on X`
- Summarizer reports → `@captain here's what the mate did`
- Mate questions → `@captain I'm stuck on Y`

Slash commands (`/merge`, `/assign`, `/cancel`) remain as action verbs — things that
do something beyond sending a message.

## Diffs on Commit

When the mate commits, the captain receives the actual diff — not a vague notification,
not a secondhand summary. This makes the captain a real code reviewer in-the-loop.

## Summarizer Overhaul

The summarizer's role is **drift detection**, not reporting.

### Buffer resets

The summarizer's context window resets on:
- Commit
- Steer (course correction from captain)

This prevents summaries based on stale activity. The summarizer only accumulates
activity since the last checkpoint.

### Silence is valid

If the mate is making steady progress, the summarizer outputs nothing. Silence means
things are fine. A message means pay attention.

### Tone

Factual compression only. No judgment, no urgency framing, no recommendations.

- Good: "Changed `auth.rs`: added token refresh logic, 47 lines"
- Bad: "URGENT: mate is rewriting the auth system without permission!"

### Trigger threshold

The current 4K token buffer fires too often, generating micro-updates about nothing.
The threshold needs to be significantly higher, and the reset-on-checkpoint behavior
means it only accumulates during periods without commits — exactly when drift detection
matters.

## Admiral

The admiral should be a valid mention target (`@admiral`), but this is blocked until
the admiral starts up properly.
