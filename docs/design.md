# cashyyc Design Document

cashyyc (Container Access Shell Helper Yielding Your Container) is a CLI tool for
interacting with devcontainer environments from the terminal. The command is `cyyc`.

## Context

devcontainer.json is a configuration format for defining reproducible development
environments. It supports features (pre-built environment components), official base images,
and multiple named environments per project. VS Code and the official devcontainer CLI use
it as the source of truth for provisioning containers.

The official CLI (`devcontainer up`, `devcontainer exec`) is designed for provisioning and
one-shot command execution. It has no model for a developer who repeatedly enters, leaves,
and re-enters the same container across a working day — the workflow of someone using a
terminal rather than an IDE.

## Motivation

The current workaround is `docker exec -it <container-id> bash`. This requires knowing the
container ID, which changes on every restart and is not human-memorable. The developer must
either memorize it, copy-paste it from `docker ps`, or write their own shell aliases. None
of these compose well with multiple environments per project.

docker-compose solved this problem for multi-container apps by introducing named services as
the unit of identity. devcontainers have the same need — a stable, named handle — but no
CLI that provides it for terminal-first use.

## Goals

- Open a shell in a devcontainer environment by name, without knowing the container ID
- Support multiple environments per project, selectable by name
- Make opening a second terminal into a running container as simple as the first

## Non-Goals

- **IDE integration**: VS Code already handles this well. cashyyc targets terminal use only.
- **SSH / remote access**: sshd setup inside containers is out of scope for MVP. It depends
  on devcontainer features configuration and adds significant complexity.
- **Full devcontainer spec support**: dockerComposeFile, lifecycle scripts, and other
  advanced fields are out of scope. The MVP targets the common case of `image` or
  `dockerfile` based environments.

## Solution

### Core abstraction: Named Environment

The unit of identity is the directory name under `.devcontainer/`. This mirrors the
devcontainer spec's own multi-environment convention and requires no new naming scheme.
Container identity is resolved via the `name` field in `devcontainer.json`, consistent with
how VS Code and the official CLI identify containers.

```
.devcontainer/
  devcontainer.json           # default (no name argument required)
  python/devcontainer.json    # selected with: cyyc shell python
  node/devcontainer.json      # selected with: cyyc shell node
```

### Interface

```
cyyc shell [name]   # Enter environment; start it first if not running
cyyc down [name]    # Stop and remove environment
cyyc ps             # List all environments and running status
```

`shell` conflates start and enter intentionally: the developer should not need to track
whether the container is already running. If no default exists and no name is given, the
CLI prints available names and exits non-zero.

`shell` always opens a new session (`docker exec -it`), never attaches to an existing one.
Multiple independent sessions into the same container are a primary use case.

## Alternative Solutions

### Wrap the official devcontainer CLI

**Pros**: Inherits full spec support, lifecycle hooks, features installation.  
**Cons**: The official CLI's `exec` subcommand is not designed for interactive use — it does
not handle TTY attachment the same way `docker exec -it` does. Wrapping it would fight the
tool rather than extend it. It also pulls in a Node.js dependency.

**Decision**: Use Docker directly. For environment provisioning (`shell` on a stopped
container), delegate to the official CLI's `up` command as a subprocess. For interactive
access, use `docker exec -it` directly.

### Add a separate `up` command

**Pros**: Explicit separation of provisioning from session entry.  
**Cons**: Forces the developer to track container state — exactly the problem this tool
exists to eliminate. Every `shell` invocation would require a prior `up` if the container
is not running. The added explicitness adds friction without safety benefit, since `shell`
on an already-running container is idempotent anyway.

**Decision**: `shell` handles provisioning transparently as a side effect.

### Go instead of Rust

**Pros**: Simpler concurrency model, faster compile times, familiar to more contributors.  
**Cons**: Go's type switch has no compile-time exhaustiveness check. For a CLI whose
correctness depends on handling every command variant, a missing case is a silent bug.
Rust's `match` enforces exhaustiveness at compile time. Rust's `Result`/`Option` chaining
also expresses the "resolve name → locate container → exec" pipeline more concisely than
Go's imperative error-check pattern.

**Decision**: Rust. Dependencies are `serde_json` (JSON parsing) and `anyhow` (error
propagation). `tokio` is explicitly excluded — `docker exec` is a subprocess call that
inherits stdin/stdout/stderr directly; there is nothing asynchronous to manage.

## Implementation Conventions

### Subprocess output control

`std::process::Command` calls are split by purpose:

- `.status()` — lets the subprocess output flow through to the terminal. Used for commands whose output is user-facing feedback (`docker start`, `docker pull`, `docker exec`).
- `.output()` — captures output for internal processing. Used when the result must be parsed by cashyyc (`docker ps` container ID extraction, `docker run` container ID extraction).

Capturing and re-emitting output from a `.status()` call distorts the native docker output. Passing it through unchanged is the more accurate user experience.

## Concerns

**macOS and WSL2 only.** Docker socket paths, TTY behavior, and process inheritance differ
across environments. Limiting to macOS and WSL2 keeps the surface area small and avoids
untested edge cases on Linux desktop or CI environments.

**Devcontainer spec coverage.** The `name` field in `devcontainer.json` is not always set.
When absent, container identity resolution needs a fallback strategy. This is an
implementation detail to be resolved during development.
