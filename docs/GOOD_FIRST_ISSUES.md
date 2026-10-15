# Good First Issue Curation

Kora's Wave initiatives are good at attracting experienced contributors to
high-complexity work, but a Wave cycle is finite. This document is the other
half: a standing, always-current pipeline of well-scoped entry points for
contributors who show up between (or after) Wave cycles. It exists so the
`good first issue` label means something specific and stays trustworthy
instead of decaying into a stale one-time list.

## The Rubric

An issue qualifies as `good first issue` when **all** of the following hold.
Use this as a checklist when labeling — not ad hoc judgment each time.

| # | Criterion | Why |
|---|---|---|
| 1 | **Touches ≤ 3 non-test files**, or is docs-only | Bounds how much of the codebase a newcomer has to hold in their head at once. |
| 2 | **No storage layout, fee logic, or access-control change** | Those require the two-maintainer + security-sign-off review path ([`CONTRIBUTING.md`](../CONTRIBUTING.md#pull-request-process)) — too much review overhead for a first PR. |
| 3 | **No open architectural decision** | If picking a solution requires a design call (which of two approaches, what a new type should look like), it's not "good first" — file it as `help wanted` instead once that call is made. |
| 4 | **Has explicit, testable acceptance criteria in the issue body** | A newcomer shouldn't have to infer "done" from a maintainer's head. |
| 5 | **Estimated at a few hours to ~2 days** for someone unfamiliar with the codebase | Anchor: reading the one relevant `contracts/<name>/src/lib.rs` file plus the fix itself, not multiple crates. |
| 6 | **Not blocked on another open issue or in-flight PR** | Avoid handing someone a dependency chain as their first contribution. |

A `good first issue` should still require a real test (per [`CONTRIBUTING.md`
§ Testing Requirements](../CONTRIBUTING.md#testing-requirements)) — "small
scope" is not "no tests."

### Worked example: sizing a real, current case

While validating [`ONBOARDING.md`](ONBOARDING.md) end-to-end, the setup walk
surfaced `contracts/financing_pool`'s compile failures (187 errors, several
files, at least one needing an actual design read on a formal-verification
model). Applying the rubric:

- Fails criterion 1 (many files) and criterion 5 (well past a few hours) as a
  single issue.
- **Not** a good first issue as filed. The right move is to split it: a
  `help wanted`-tier triage issue first ("identify and categorize the
  `financing_pool` compile errors"), then, once categorized, individual
  mechanical fixes (a missing enum variant, a bad import) get filed as
  separate `good first issue`s, while anything touching the verification
  model or fee logic stays `help wanted`.

This is the intended workflow: large, real breakage becomes a set of small,
rubric-passing issues rather than one issue nobody can size.

## Where Candidates Come From

Curation is ongoing, not a one-time sweep. Sources, roughly in order of how
often they produce good candidates:

1. **CI failures with a narrow, obvious cause** — a single missing enum
   variant, an unregistered module, an unused import treated as an error by
   `-D warnings`. These are usually one-file, one-concept fixes.
2. **`cargo run -p kora-xtask --bin check-error-variants` findings** —
   references to undeclared `KoraError` variants are close to auto-qualifying
   good first issues (criteria 1–3 are satisfied by construction).
3. **Coverage gaps** flagged by `make coverage-per-contract` (see
   [`Makefile`](../Makefile) — issue #683's per-contract floors) — "add a
   failure-path test for `X`" is close to ideal shape.
4. **Doc/code drift** — a function's doc comment describing behavior the code
   no longer has, or a `docs/*.md` file describing an entrypoint whose
   signature has changed.
5. **`TODO.md`** and inline `// TODO` comments, filtered through the rubric
   above (most are not scoped small enough as-is; split them the same way as
   the financing_pool example).

When triaging any of the above into an issue, use the standard issue
template and explicitly write the acceptance criteria (rubric item 4) — don't
just paste a stack trace.

## Keeping the List Current

A curated list that never gets revisited is worse than no list — it wastes a
newcomer's time on a `good first issue` that's already stale, half-fixed, or
silently abandoned. `.github/workflows/good-first-issue-curation.yml` runs
weekly and:

- Flags any `good first issue` with no activity (no comments, no linked PR)
  for **45+ days** as needing a curator look — is it still accurate, still
  unclaimed, still scoped right?
- Flags any `good first issue` that's **assigned** but has had no linked PR
  activity for 45+ days — a likely dropped claim, freeing it back up.
- Posts (or updates) a single tracking issue titled **"Good First Issue
  Curation: Needs Review"** listing everything flagged, the same
  upsert-a-tracking-issue pattern used by
  [`dependency-advisories.yml`](../.github/workflows/dependency-advisories.yml).
- Separately flags when the **open, unassigned** `good first issue` count
  drops to zero — an empty pipeline is exactly the failure mode this
  document exists to prevent.

Maintainers triage that tracking issue like any other: re-confirm scope and
close-and-relabel, reassign, or unassign a stale claim. Run it manually via
**Actions → Good First Issue Curation → Run workflow** to check the current
state without waiting for the schedule.

## Recognition

Completing a `good first issue` gets you into [`CONTRIBUTORS.md`](../CONTRIBUTORS.md)
same as any other merged PR — see [`CONTRIBUTING.md` §
Recognition](../CONTRIBUTING.md#recognition). It's also the fastest way to
build the track record that makes a Wave application (see
[`docs/wave-dashboard.md`](wave-dashboard.md)) easy to say yes to.

## Curated Candidate Pipeline (Issue #893 Resolution)

In response to tracking issue #893 identifying an empty curation pipeline, the following candidate issues have been triaged against the rubric and are ready for assignment:

### Candidate 1: Standardize unused emit helpers in financing pool
- **Rubric check**: Touches 1 file (`contracts/financing_pool/src/lib.rs`), no storage layout change, clear acceptance criteria, ~2-4 hours.
- **Scope**: Clean up deprecated/redundant local emit helpers and ensure all financing pool event emissions reference `kora_shared::events::*`.
- **Criteria**: Remove duplicate emit functions; confirm zero warnings in `financing_pool`.

### Candidate 2: Document error variant mappings in xtask
- **Rubric check**: Touches 1 file (`contracts/xtask/src/main.rs` or docs), mechanical update, no architectural decision, ~2 hours.
- **Scope**: Add documentation and missing error code documentation discovered by `check-error-variants`.
- **Criteria**: Update check-error-variants helper output with descriptive troubleshooting hints.

### Candidate 3: Fix doc/code drift in access control views
- **Rubric check**: Touches 1 doc file (`docs/access-control.md`), docs-only, ~1 hour.
- **Scope**: Ensure doc comments in `docs/access-control.md` match public view functions in `contracts/access_control/src/lib.rs`.
- **Criteria**: Align function parameter lists and return error enums.

