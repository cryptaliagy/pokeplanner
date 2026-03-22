# Multi-Session LLM Container Architecture Research

> Research conducted 2026-03-22. Covers the current state of tooling for running
> multiple concurrent LLM coding agent sessions on the same repository in
> isolated environments.

## Table of Contents

- [1. Existing Tooling](#1-existing-tooling)
- [2. Model & Software Support Matrix](#2-model--software-support-matrix)
- [3. Nix Cache Integration in Containers](#3-nix-cache-integration-in-containers)
- [4. Architecture for a Custom System](#4-architecture-for-a-custom-system)

---

## 1. Existing Tooling

### Tier 1: Purpose-Built for This Exact Problem

#### Container Use (by Dagger)

Open-source MCP server + CLI that gives each agent its own **container + git worktree**. This is the closest existing tool to the "spawn a container with tooling and a fresh repo copy" vision.

- Each agent gets a Docker container on its own git branch
- Works with any MCP-compatible agent (Claude Code, Cursor, Zed, etc.)
- View command history/logs, drop into any agent's terminal, review via `git checkout`
- Install: `brew install dagger/tap/container-use` (macOS) or install script (Linux)
- Still early-stage but actively developed

Links:
- [GitHub: dagger/container-use](https://github.com/dagger/container-use)
- [Blog: Containing Agent Chaos](https://dagger.io/blog/agent-container-use/)
- [Zed + Container Use for Background Agents](https://zed.dev/blog/container-use-background-agents)

#### Claude Code Native Worktree Support

As of Claude Code v2.1.49+, native git worktree support is built into the CLI:

```bash
# Launch a session in its own worktree (creates .claude/worktrees/<name>/)
claude --worktree feature-auth

# Launch in a worktree AND a background tmux session
claude --worktree feature-auth --tmux

# Headless mode for scripting parallel sessions
claude -p "implement feature X" --output-format stream-json
```

Sub-agents also support `isolation: "worktree"` for automatic worktree creation/cleanup.

Worktrees are cleaned up automatically if no changes are made; if changes exist, the user is prompted.

#### Claude Agent SDK

TypeScript (v0.2.71) and Python (v0.1.48) SDKs for programmatic orchestration:

- Spawn up to 10 specialized sub-agents working in parallel
- `spawn_claude_code_process` allows running agents in VMs, containers, or remote environments
- Full control over system prompts, allowed tools, max turns

Links:
- [Agent SDK Overview](https://platform.claude.com/docs/en/agent-sdk/overview)
- [GitHub: anthropics/claude-agent-sdk-typescript](https://github.com/anthropics/claude-agent-sdk-typescript)

#### `par` CLI

A dedicated CLI for parallel worktree + session management, designed specifically for AI coding assistants:

- [GitHub: coplane/par](https://github.com/coplane/par)

### Tier 2: IDE-Integrated Parallel Agents

#### Cursor 2.0 (Oct 2025)

- Up to 8 simultaneous agents, each in isolated git worktrees or remote machines
- Distributes prompts to all selected models simultaneously

#### Windsurf Wave 13

- Parallel Cascade sessions with git worktree branch isolation
- Multi-pane layout, dedicated terminal profile for agent execution

### Tier 3: Container Sandbox Platforms

These provide the runtime/sandbox layer but not the LLM orchestration:

| Platform | Isolation | Boot Time | License |
|----------|-----------|-----------|---------|
| **OpenHands** (fmr. OpenDevin) | Docker containers w/ SSH | Seconds | MIT |
| **E2B** | Firecracker microVMs | ~200ms | Apache-2.0 |
| **Daytona** | Docker containers | <90ms | AGPL-3.0 |
| **CodeSandbox** | microVMs | ~2s | Proprietary |

**OpenHands** is the most relevant here — each agent runs in a Docker sandbox, supports any LLM provider, and achieves 72% on SWE-Bench Verified. They run 32x parallel sessions for evaluation.

### Tier 4: Orchestration Tools (Manage Multiple Agents)

These tools don't provide isolation themselves but manage the lifecycle of multiple parallel agent sessions:

| Tool | Description |
|------|-------------|
| **[dmux](https://github.com/anthropics/dmux)** | Node.js CLI, creates tmux panes with per-agent git worktrees; supports Claude Code, Codex, OpenCode, Gemini CLI |
| **[cmux](https://github.com/anthropics/cmux)** | macOS-focused multi-agent manager (7.7k GitHub stars in first month, Feb 2026) |
| **[NTM](https://github.com/anthropics/ntm)** | Named Tmux Manager — named panes, broadcast prompts, conflict detection, TUI dashboard |
| **[agentree](https://github.com/anthropics/agentree)** | Auto-creates git worktrees per agent session |
| **[Vibe Kanban](https://github.com/anthropics/vibe-kanban)** | Kanban board + worktree isolation + multi-agent (9.4k stars) |
| **Superset** | Electron IDE for 10+ parallel agents |

### Tier 5: Lightweight (No Containers, No Orchestrator)

#### Aider

- No built-in worktree or multi-session support (single-threaded by design)
- [Feature request #4428](https://github.com/aider-ai/aider/issues/4428) proposes `/spawn`, `/delegate` commands
- Works in manually created worktrees; supports many models (Claude, GPT-4, Gemini, local via Ollama)

#### Manual Worktree + Headless Claude

The simplest viable approach:

```bash
# Create worktrees
git worktree add /tmp/session-1 -b feature/task-1
git worktree add /tmp/session-2 -b feature/task-2

# Run Claude Code headless in each (parallel)
(cd /tmp/session-1 && claude -p "implement X" --output-format stream-json) &
(cd /tmp/session-2 && claude -p "implement Y" --output-format stream-json) &
wait
```

Or with the built-in worktree flag:

```bash
# Built-in worktree + tmux (simplest of all)
claude -w feature-auth --tmux
claude -w bugfix-123 --tmux
claude -w refactor-api --tmux
```

### Claude Agent SDK (Programmatic Control)

For building a custom orchestrator, the Claude Agent SDK exposes the full agent loop as a library:

```typescript
import { query } from "@anthropic-ai/claude-agent-sdk";

for await (const msg of query({
  prompt: "Add error handling to the API routes",
  options: {
    model: "claude-sonnet-4-20250514",
    allowedTools: ["Read", "Edit", "Bash", "Glob"],
    permissionMode: "acceptEdits",
    systemPrompt: "You are a senior backend engineer.",
  }
})) {
  if (msg.type === "result") console.log(msg.content);
}
```

Key features: session forking (`forkSession`), file checkpointing with rollback, inline subagent definitions, `canUseTool` callback for fine-grained permissions, MCP tool support. A V2 preview simplifies multi-turn conversations with a `send()`/`stream()` API.

Available on npm (`@anthropic-ai/claude-agent-sdk` v0.2.71) and PyPI (`claude-agent-sdk` v0.1.48).

### Practical Limits

The consensus across the ecosystem is **5-7 concurrent agents on a single machine** before rate limits, merge conflict overhead at review time, and cognitive load dominate. The workflow shifts from "writer" to "reviewer" — you decompose work, launch agents, and spend time validating output. Teams like incident.io report 4-5 parallel instances as their default workflow; Citadel reported only a 3.1% conflict rate across 109 parallel agent waves.

---

## 2. Model & Software Support Matrix

| Tool | Supported Models |
|------|-----------------|
| **Claude Code / Agent SDK** | Claude (Opus, Sonnet, Haiku) only |
| **Cursor** | Claude, GPT-4o, Gemini, others (model-agnostic) |
| **Windsurf** | Claude, GPT, Gemini, SWE-1 |
| **OpenHands** | Any LLM provider (Claude, GPT, Gemini, open-source) |
| **Aider** | Any LLM with an API (Claude, GPT, Gemini, Llama, DeepSeek, Mistral, etc.) |
| **Container Use** | Agent-agnostic (provides container/worktree layer, works with any MCP agent) |
| **E2B / Daytona** | Runtime-only (model-agnostic, they provide the sandbox) |
| **SWE-agent** | Primarily Claude/GPT; others via LiteLLM |

---

## 3. Nix Cache Integration in Containers

### Option A: Bind-Mount Host Nix Store

```bash
docker run \
  -v /nix/store:/nix/store:ro \
  -v /nix/var/nix/db:/nix/var/nix/db:ro \
  -e PATH="/nix/store/...-my-env/bin:$PATH" \
  my-agent-image
```

**Caveats**:

- **Read-only substituter bug** ([NixOS/nix#6835](https://github.com/NixOS/nix/issues/6835)): When Nix inside the container tries to use a read-only mounted store as a local substituter, it attempts to remount the store as writable, fails with "Operation not permitted," and falls back to fetching from cache.nixos.org.
- **runc 1.2+ regression** ([opencontainers/runc#4575](https://github.com/opencontainers/runc/issues/4575)): `MS_REMOUNT` on bind mounts can cause read-only volume mounts to fail on recent container runtimes.
- **User namespaces**: In rootless Docker/Podman, UID mapping can prevent access to store paths owned by root. Use `--userns=keep-id` (Podman) or ensure the container user has read access.

Workarounds:

- Mount read-**write** for ephemeral containers (they're throwaway anyway)
- Mount at an alternate path (e.g., `/host-nix-store`) and use as `--substituters file:///host-nix-store` rather than as the primary store
- Use an **overlayfs** with `/nix/store` as the read-only lower layer and a writable upper layer for new builds:
  ```bash
  mount -t overlay overlay \
    -o lowerdir=/host-nix-store,upperdir=/nix-upper,workdir=/nix-work /nix/store
  ```
- Run `nix-daemon` on host, share the socket into containers

### Option B: Local Binary Cache with nix-serve

```bash
# On host — serve the Nix store over HTTP
nix-serve --port 5000 &

# In container's nix.conf
substituters = http://host.docker.internal:5000
trusted-public-keys = <your-key>
```

Containers pull pre-built derivations from the host on demand. No full store mount needed.

### Option C: Attic (Self-Hosted, More Capable)

[Attic](https://github.com/zhaofengli/attic) is a more feature-rich alternative to nix-serve:

- S3-compatible storage backend
- Multi-tenant with access control
- Global deduplication and garbage collection
- Better suited for teams/CI

### Option D: Export a Closure

```bash
# Export just the dev shell closure
nix copy --to file:///shared-cache $(nix build .#devShell --print-out-paths)

# In container
nix copy --from file:///shared-cache /nix/store/...-devshell
```

### Option E: Devenv / Devbox Container Generation

Both tools can produce container images with the Nix environment baked in:

```bash
# devenv — builds an OCI image with your full dev environment
devenv container build shell

# devbox — generates a Dockerfile that uses Nix
devbox generate dockerfile
```

These integrate with Cachix / Jetify Cache for binary caching, so builds pull pre-compiled packages rather than compiling from source.

### Option F: Nix-Built Container Images (No Dockerfile)

For production-quality minimal images, `dockerTools.buildLayeredImage` creates one Docker layer per store path, enabling efficient layer caching:

```nix
pkgs.dockerTools.buildLayeredImage {
  name = "pokeplanner-session";
  tag = "latest";
  contents = [ myDevShell ];
  config.WorkingDir = "/workspace";
}
```

For fastest rebuild/push cycles, [nix2container](https://github.com/nlewo/nix2container) avoids writing tarballs to the Nix store entirely and skips already-pushed layers.

### Option G: Named Docker Volume for `/nix` (CI/Dev)

For iterative dev where you want the Nix store to persist across container runs:

```bash
docker run -v nix-store:/nix myimage nix build .#myapp
```

The named volume persists the store, so subsequent builds are fast. Simplest caching approach for CI.

### Recommended Approach

For the "spawn N agent containers quickly" use case:

1. **Build a base image** via `devenv container build` or `nix build .#dockerImage` — bakes in the full toolchain (Rust, protobuf, etc.)
2. **Run nix-serve or Attic** on the host for any additional derivations
3. **Bind-mount `/nix/store` read-write** into ephemeral containers for maximum speed
4. **Each container gets**: base image + worktree mount (rw) + Nix store mount (ro/rw) + shared cargo registry (ro)

---

## 4. Architecture for a Custom System

### Component Diagram

```
┌──────────────────────────────────────────────────┐
│                Session Orchestrator               │
│  (CLI tool or small daemon)                       │
│                                                   │
│  Commands:                                        │
│    spawn <prompt> [--branch name]                 │
│    list                                           │
│    attach <session-id>                            │
│    kill <session-id>                              │
│    status                                         │
│                                                   │
│  Responsibilities:                                │
│    1. git worktree add for each session           │
│    2. Launch container with worktree mounted      │
│    3. Start LLM agent inside container            │
│    4. Stream logs / allow attach                  │
│    5. On completion: report branch + diff         │
└──────────────┬───────────────────────────────────┘
               │
    ┌──────────┼──────────┐
    ▼          ▼          ▼
┌────────┐ ┌────────┐ ┌────────┐
│  Ctr 1 │ │  Ctr 2 │ │  Ctr 3 │  ← Containers (Podman/Docker)
│        │ │        │ │        │
│ Mounts:│ │        │ │        │
│  • worktree (rw)  │ │        │
│  • /nix/store     │ │        │
│  • cargo cache    │ │        │  ← Shared caches
│        │ │        │ │        │
│ Runs:  │ │ Runs:  │ │ Runs:  │
│ claude │ │ claude │ │ aider  │  ← Any agent
│  -p .. │ │  -p .. │ │        │
└───┬────┘ └───┬────┘ └───┬────┘
    │          │          │
    ▼          ▼          ▼
  branch/    branch/    branch/
  task-1     task-2     task-3     ← Each on its own branch
```

### Component Details

#### 1. Session Orchestrator

A small CLI (bash script, Python, or Rust) that manages the lifecycle:

```bash
#!/usr/bin/env bash
# spawn-session.sh
SESSION_ID=$(uuidgen | head -c 8)
BRANCH="agent/session-${SESSION_ID}"
WORKTREE="/tmp/sessions/${SESSION_ID}"

git worktree add "$WORKTREE" -b "$BRANCH"

podman run --rm -d \
  --name "session-${SESSION_ID}" \
  -v "$WORKTREE":/workspace:rw \
  -v /nix/store:/nix/store:rw \
  -v /nix/var/nix/db:/nix/var/nix/db:ro \
  -v "${CARGO_HOME:-$HOME/.cargo}/registry":/cargo-cache:ro \
  -e ANTHROPIC_API_KEY="$ANTHROPIC_API_KEY" \
  pokeplanner-session:latest \
  claude -p "$1" --output-format stream-json
```

#### 2. Container Image (Nix-built)

```nix
# In flake.nix
packages.sessionImage = pkgs.dockerTools.buildImage {
  name = "pokeplanner-session";
  copyToRoot = pkgs.buildEnv {
    name = "session-env";
    paths = with pkgs; [
      bash git curl cacert
      rustc cargo
      protobuf grpcurl
      # claude-code via npm or binary
    ];
  };
  config.WorkingDir = "/workspace";
};
```

Or use `devenv container build shell` if you already have a `devenv.nix`.

#### 3. Shared Caches (mounted into all containers)

| Mount | Mode | Purpose |
|-------|------|---------|
| `/nix/store` | rw (ephemeral) or ro | Pre-built Nix derivations |
| `~/.cargo/registry` | ro | Downloaded crate sources |
| sccache dir | rw (shared) | Compiled artifact cache |

#### 4. Merge/Review Pipeline

Each agent works on its own branch. On completion, the orchestrator:
- Runs CI on each branch
- Auto-merges non-conflicting changes, or
- Creates PRs for human review
- Flags conflicts for manual resolution

#### 5. Observability

- Real-time log streaming per session (Container Use's model)
- Ability to attach to a running session's terminal
- Token usage and cost tracking per agent
- Dashboard (optional) showing all active sessions

### Simplest Starting Point

If you want to start experimenting today without building anything:

```bash
# Terminal 1
git worktree add /tmp/s1 -b agent/task-1
cd /tmp/s1 && claude -p "implement feature X"

# Terminal 2
git worktree add /tmp/s2 -b agent/task-2
cd /tmp/s2 && claude -p "implement feature Y"

# Terminal 3 — you review as branches complete
git log --oneline --all --graph
```

Add containers when you need dependency isolation or resource limits. Add Nix store sharing when container startup time matters.

### Recommended Existing Tool

**Dagger Container Use** is the closest turnkey solution to what you described. It combines containers + worktrees + agent orchestration in a single tool, works with Claude Code via MCP, and is open source. It's the fastest path to a working setup without building custom infrastructure.

---

## Sources

- [Dagger Container Use](https://github.com/dagger/container-use)
- [Claude Code Worktree Docs](https://code.claude.com/docs/en/common-workflows)
- [Claude Agent SDK](https://platform.claude.com/docs/en/agent-sdk/overview)
- [OpenHands](https://github.com/OpenHands/OpenHands)
- [par CLI](https://github.com/coplane/par)
- [NixOS/nix#6835 — read-only store mount issue](https://github.com/NixOS/nix/issues/6835)
- [nix-serve](https://github.com/edolstra/nix-serve)
- [Attic self-hosted binary cache](https://github.com/zhaofengli/attic)
- [Devenv containers](https://devenv.sh/containers/)
- [Devbox Docker integration](https://www.jetify.com/blog/creating-nix-powered-docker-containers-with-devbox)
- [E2B](https://e2b.dev/)
- [Daytona](https://daytona.io/)
- [Agentmaxxing blog post](https://vibecoding.app/blog/agentmaxxing)
- [Simon Willison: Parallel Coding Agents](https://simonwillison.net/2025/Oct/5/parallel-coding-agents/)
- [Ona: Run Claude Code in Parallel](https://ona.com/stories/parallelize-claude-code)
