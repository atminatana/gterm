---
name: init
description: Initialize or refresh a repository for AI coding agents — survey the actual files, then create or update AGENTS.md plus the companion files that make agents obey it (CLAUDE.md import pointer, scoped sub-package AGENTS.md, stack-specific .gitignore). Use when the user runs /init, says "init this project/repo", "set up agent context", "bootstrap this repo", "инициализируй проект", asks to create/update AGENTS.md or CLAUDE.md, or when real work is about to start in a repo that has no AGENTS.md. Not for writing product code, doc requests, or PR review.
---

# init

## Purpose

Make the current repository legible to a coding agent in one pass: an evidence-based survey of the files, turned into a thin, high-signal `AGENTS.md` and the small set of companion files that make agents follow it.

Re-runnable by design — it updates what it owns and never clobbers what it does not.

## Triggers

- `/init`, "init this project", "set up agent context", "bootstrap this repo", "инициализируй проект"
- "create / write / update AGENTS.md", "создай AGENTS.md", "сделай CLAUDE.md"
- Real work is about to start in a repository that has no `AGENTS.md`

Not this skill: writing product code, one-off documentation requests, reviewing a pull request.

## Scope

**Owns:** `AGENTS.md`, the `CLAUDE.md` import pointer, scoped `AGENTS.md` in sub-packages, `.gitignore` entries for the detected stack, and `.agents/skills/` if it is missing.

**Never:** `git commit` / `git push` / history rewriting, installing dependencies or global tools, touching source code, CI config, or dependency versions, creating a `README.md` unless asked. Anything outside this list is a proposal to the user, not an action.

---

## Workflow

### 1. Inventory the existing agent setup (idempotency guard)

Read before writing anything — repo root:

- `AGENTS.md`, `CLAUDE.md`, `PROJECT_MAP.md`, `.cursorrules`, `GEMINI.md`, `copilot-instructions.md`
- `.claude/`, `.agents/`, `.codex/`, `.github/copilot-instructions.md`

For each hit, decide:

| State | Action |
|---|---|
| Exists with the marker `<!-- managed-by: init -->` | Regenerate content, keep the marker, report what changed |
| Exists without the marker | **Stop and ask**: update in place (with a diff summary), merge, or skip. Never overwrite silently |
| `CLAUDE.md` containing only `@AGENTS.md` | Treat as the pointer, not as user content |
| `AGENTS.override.md` present | It wins over `AGENTS.md` — read it and do not create a conflicting file |

### 2. Detect the stack — from files, not guesses

Check in order, stop at the first hit; record the version from the manifest or lockfile:

| Manifest | Stack | Primary commands |
|---|---|---|
| `Cargo.toml` | Rust | `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt` |
| `package.json` + lockfile | Node / TS | install via the lockfile's package manager, `npm|pnpm|yarn|bun test`, `... run lint` |
| `pyproject.toml` / `requirements.txt` | Python | `pytest`, `ruff check .`, `mypy .` |
| `go.mod` | Go | `go build ./...`, `go test ./...`, `golangci-lint run` |
| `pom.xml` / `build.gradle*` | Java | `mvn test` / `./gradlew test` |
| `*.csproj` / `*.sln` | .NET | `dotnet build`, `dotnet test` |
| `CMakeLists.txt` | C / C++ | `cmake --build build`, `ctest --test-dir build` |
| none | unknown | Ask the user; do not guess a stack |

Determine the package manager from the lockfile that is actually present — an agent that runs `npm install` in a `pnpm-lock.yaml` repo breaks the lockfile.

### 3. Survey — read, never infer

Read these (modify nothing):

- the manifest and lockfile: package name, versions, workspace/package layout
- top-level tree, one level deep — skip `node_modules`, `target`, `vendor`, `dist`, `build`, `.git`
- entry points: `src/main.rs`, `src/index.ts`, `cmd/*/main.go`, `manage.py`, `Program.cs`
- CI: `.github/workflows/*.yml`, `.gitlab-ci.yml`, `Jenkinsfile`, `azure-pipelines.yml` — whatever gates CI enforces *are* this project's quality gates
- test layout and runner config: `tests/`, `__tests__/`, `pytest.ini`, `vitest.config.*`, `[dev-dependencies]`
- existing docs: `README.md`, `CONTRIBUTING.md`, `docs/`
- history: `git log --oneline -20` for commit style, `git branch -a` for branch naming, any PR template
- `.gitignore` for what is already ignored

Then report to the user, briefly:

- one paragraph: what this project is and what it does
- stack with versions
- the verified command table
- anything non-obvious that you only learned by reading the code

### 4. Plan and confirm

Present a numbered list of the files to create or change, one line of reason each, and wait for approval. Creating files the user did not ask for is a scope decision, not a detail.

### 5. Write the root `AGENTS.md`

Target **30–60 lines**; hard maximum 300. Sections in priority order:

1. **Purpose** — one paragraph
2. **Stack** — language, framework, versions
3. **Commands** — exact, copy-paste ready, with flags
4. **File-scoped commands** — per-file test/lint/typecheck, cheaper than a full build
5. **Structure** — annotated tree, real paths only
6. **Conventions** — cite `path:line` examples from this repo, not invented snippets
7. **Boundaries** — `Always` / `Ask first` / `Never`
8. **Gotchas** — env vars, generated files, surprising constraints

Do not embed code snippets — they go stale. Reference `path:line`. Do not tell the agent to read `AGENTS.md` or `CLAUDE.md`; they are already loaded.

### 6. `CLAUDE.md` pointer

Claude Code reads `CLAUDE.md`, Codex and others read `AGENTS.md`. Keep one source of truth:

- `CLAUDE.md` absent → write a file whose only line is `@AGENTS.md`
- `CLAUDE.md` has user content → prepend `@AGENTS.md`, keep the rest as Claude-specific extension (hooks, MCP servers, permissions, skill routing)
- On Windows, prefer this import over `New-Item -ItemType SymbolicLink` — symlinks need Developer Mode or elevation and break silently on clone. Offer the symlink only if the user asks for one.

### 7. Scoped sub-files

Create `AGENTS.md` inside a sub-package **only** when that directory has its own commands, stack, or conventions that diverge from the root. Sub-files carry more detail than the root, and never duplicate it. In a single-module repo this step is normally a no-op — say so rather than inventing files.

### 8. `.gitignore`

Add only what the detected stack is missing, and only for real build output: Rust `target/`, Node `node_modules/` + the local log file, Python `__pycache__/` + `.venv/`, Go binaries, IDE directories. Never secrets-adjacent patterns the project already handles. Never remove existing entries.

### 9. Verify — before reporting anything

- every path named in the generated files exists (check each one, do not eyeball)
- every documented command's tool exists on the machine (`Get-Command cargo`, `Get-Command git`; `npx --no-install` for JS)
- no placeholders survive: `TODO`, `TBD`, `<...>`, `FIXME`
- root file is inside the line budget; no duplication between root and sub-files
- `git status --short` shows only the intended files

Report anything you could not verify as an explicit gap. Never state an unverified command works.

### 10. Report

List the files created or changed, one line each. Then state what an agent can now do that it could not before, and list the open questions you could not answer from the repo. Do not commit — the user commits.

---

## Quality bar

A line survives into `AGENTS.md` only if an agent would plausibly get it **wrong** without it.

**Include:** exact commands with flags, how to run a single test, required env vars and services, quality gates CI enforces, non-obvious constraints, boundaries and prohibitions.

**Exclude:** anything readable from the manifest, obvious from the tree, generic best practice a competent agent already applies, restated stack trivia, aspirational rules the project does not follow.

If unsure whether a line earns its place — cut it. It can be added later when a real problem shows up.

## Anti-patterns

- Fabricated commands, versions, or paths — every fact must come from a file you read
- A 300-line `AGENTS.md` padded with restated stack info
- Overwriting an existing agent file without asking
- Silently installing tools, adding dependencies, or editing CI
- Committing on the user's behalf
- Writing a rule the project does not actually follow

## Platform notes (Windows / PowerShell)

- Prefer `Get-ChildItem`, `Get-Content`, `Test-Path`, `Select-String` over `ls`/`cat`/`grep`; both work but the cmdlets do not trip on alias surprises.
- Create directories with `New-Item -ItemType Directory -Force`.
- Line counts: `(Get-Content <file>).Count`.
- Rust/Node/Python toolchains may not be on `PATH` — if a command cannot be resolved, say so in the report instead of guessing its output.