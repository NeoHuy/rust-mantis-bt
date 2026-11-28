#!/usr/bin/env python3
"""Antigravity lifecycle hook for automated AI code review.

Supports SessionStart, PostToolUse (file edit tracking), and Stop (review).
Default reviewer provider is `claude` CLI, configurable to `codex` or `custom`
via `.agents/ai-review-config.json` or environment variables.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
AGENTS_DIR = SCRIPT_DIR.parent
DEFAULT_CONFIG_PATH = AGENTS_DIR / "ai-review-config.json"
STATE_DIR = Path(tempfile.gettempdir()) / f"agy-ai-review-{os.getuid()}"

DEFAULT_CONFIG: dict[str, Any] = {
    "enabled": True,
    "provider": "claude",
    "model": "",
    "reviewer_by_runtime": {
        "claude": {"provider": "codex", "model": ""},
        "codex": {"provider": "claude", "model": ""},
    },
    "max_review_rounds": 5,
    "include_nits": True,
    "timeout_seconds": 120,
    "max_diff_chars": 60000,
    "custom_command": [],
    "ignore_patterns": [
        "Cargo.lock",
        "package-lock.json",
        "target/*",
        "*/target/*",
        "node_modules/*",
        "*/node_modules/*",
        ".git/*",
        "__pycache__/*",
        "*/__pycache__/*",
        "*.pyc",
        "*.min.js",
        "*.min.css",
        "*.png",
        "*.jpg",
        "*.jpeg",
        "*.gif",
        "*.ico",
        "*.svg",
        "*.woff*",
        "*.pdf",
        "*.db",
        "*.db-shm",
        "*.db-wal",
    ],
    "extra_instructions": "",
}

VERDICT_RE = re.compile(
    r"(?im)^\s*(?:\*\*)?VERDICT\s*:\s*(PASS|NEEDS_FIX)(?:\*\*)?\b"
)
APPLY_PATCH_FILE_RE = re.compile(
    r"^\*\*\*\s+(?:Add File|Update File|Delete File|Move to):\s*(.+?)\s*$",
    re.MULTILINE,
)


def _truthy_env(name: str) -> bool:
    val = os.environ.get(name, "").strip().lower()
    return val in {"1", "true", "yes", "on"}


def _safe_int(value: Any, default: int, minimum: int) -> int:
    try:
        if value is None or isinstance(value, bool):
            return default
        return max(minimum, int(value))
    except (ValueError, TypeError):
        return default


def load_config(config_path: Path | None = None) -> dict[str, Any]:
    """Load config from JSON file and apply environment variable overrides."""
    cfg = dict(DEFAULT_CONFIG)
    merged_rt = {
        k: dict(v) for k, v in DEFAULT_CONFIG["reviewer_by_runtime"].items()
    }
    explicit_runtimes: list[str] = []
    path = config_path or DEFAULT_CONFIG_PATH
    if path.is_file():
        try:
            with path.open("r", encoding="utf-8") as fh:
                loaded = json.load(fh)
            if isinstance(loaded, dict):
                cfg.update(loaded)
                if isinstance(loaded.get("reviewer_by_runtime"), dict):
                    for rt_key, rt_val in loaded["reviewer_by_runtime"].items():
                        if isinstance(rt_val, dict):
                            base_entry = dict(merged_rt.get(rt_key, {}))
                            base_entry.update(rt_val)
                            merged_rt[rt_key] = base_entry
                            explicit_runtimes.append(str(rt_key))
        except Exception as exc:
            print(f"[ai-code-review] Failed to parse {path}: {exc}", file=sys.stderr)
    cfg["reviewer_by_runtime"] = merged_rt
    cfg["_explicit_runtimes"] = explicit_runtimes

    if _truthy_env("AI_REVIEW_DISABLED"):
        cfg["enabled"] = False
    if os.environ.get("AI_REVIEW_PROVIDER"):
        cfg["provider"] = os.environ["AI_REVIEW_PROVIDER"].strip().lower()
    if "AI_REVIEW_MODEL" in os.environ and os.environ["AI_REVIEW_MODEL"].strip():
        cfg["model"] = os.environ["AI_REVIEW_MODEL"].strip()
    if os.environ.get("AI_REVIEW_MAX_ROUNDS"):
        cfg["max_review_rounds"] = _safe_int(os.environ["AI_REVIEW_MAX_ROUNDS"], 5, 1)
    if "AI_REVIEW_INCLUDE_NITS" in os.environ:
        env_nits = os.environ["AI_REVIEW_INCLUDE_NITS"].strip().lower()
        if env_nits in {"1", "true", "yes", "on"}:
            cfg["include_nits"] = True
        elif env_nits in {"0", "false", "no", "off"}:
            cfg["include_nits"] = False
    if os.environ.get("AI_REVIEW_TIMEOUT"):
        cfg["timeout_seconds"] = _safe_int(os.environ["AI_REVIEW_TIMEOUT"], 120, 10)

    cfg["provider"] = str(cfg.get("provider", "claude")).strip().lower() or "claude"
    cfg["model"] = str(cfg.get("model", "")).strip()
    cfg["max_review_rounds"] = _safe_int(cfg.get("max_review_rounds"), 5, 1)
    cfg["include_nits"] = bool(cfg.get("include_nits", True))
    cfg["timeout_seconds"] = min(150, _safe_int(cfg.get("timeout_seconds"), 120, 10))
    cfg["max_diff_chars"] = _safe_int(cfg.get("max_diff_chars"), 60000, 1000)
    if not isinstance(cfg.get("ignore_patterns"), list):
        cfg["ignore_patterns"] = list(DEFAULT_CONFIG["ignore_patterns"])
    if not isinstance(cfg.get("custom_command"), list):
        cfg["custom_command"] = []
    return cfg


def detect_runtime(payload: dict[str, Any], requested: str = "auto") -> str:
    """Resolve active agent runtime from CLI flag or stdin payload shape."""
    if requested and requested != "auto":
        return requested
    if "conversationId" in payload or "hookEventName" in payload:
        return "antigravity"
    if (
        "conversation_id" in payload
        or "workspace_roots" in payload
        or "loop_count" in payload
    ):
        return "cursor"
    if "turn_id" in payload or payload.get("tool_name") == "apply_patch":
        return "codex"
    if "session_id" in payload or "stop_hook_active" in payload:
        return "claude"
    return "antigravity"


def extract_session_id(payload: dict[str, Any], runtime: str) -> str:
    """Extract normalized session key namespaced by runtime."""
    raw = "default"
    for key in ("conversationId", "session_id", "conversation_id", "generation_id"):
        val = payload.get(key)
        if isinstance(val, str) and val.strip():
            raw = val.strip()
            break
    return f"{runtime}:{raw}"


def resolve_reviewer_config(config: dict[str, Any], runtime: str) -> dict[str, Any]:
    """Select reviewer provider/model for the calling agent runtime."""
    resolved = dict(config)
    env_provider = os.environ.get("AI_REVIEW_PROVIDER", "").strip().lower()
    env_model = os.environ.get("AI_REVIEW_MODEL", "").strip()

    if env_provider:
        return resolved

    rt_map = config.get("reviewer_by_runtime")
    rt_entry = rt_map.get(runtime) if isinstance(rt_map, dict) else None
    explicit_runtimes = set(config.get("_explicit_runtimes") or [])
    is_explicit_rt = runtime in explicit_runtimes
    is_same_provider_guard = resolved["provider"] == runtime

    if isinstance(rt_entry, dict) and (is_explicit_rt or is_same_provider_guard):
        rt_prov = str(rt_entry.get("provider", "")).strip().lower()
        if rt_prov:
            resolved["provider"] = rt_prov
            if not env_model:
                rt_model = str(rt_entry.get("model", "")).strip()
                if rt_model:
                    resolved["model"] = rt_model
                elif rt_prov != config.get("provider"):
                    resolved["model"] = ""
    return resolved


def allow_response(runtime: str, action: str) -> dict[str, Any]:
    """Return runtime-appropriate allow/no-op response."""
    if action != "review":
        return {}
    if runtime == "antigravity":
        return {"decision": "allow"}
    return {}


def block_response(runtime: str, reason: str) -> dict[str, Any]:
    """Return runtime-appropriate continuation/block response."""
    if runtime == "antigravity":
        return {"decision": "continue", "reason": reason}
    if runtime == "cursor":
        return {"followup_message": reason}
    return {"decision": "block", "reason": reason}


def read_stdin_json() -> dict[str, Any]:
    """Read JSON payload from stdin, returning empty dict on empty/invalid input."""
    try:
        raw = sys.stdin.read()
        if not raw.strip():
            return {}
        data = json.loads(raw)
        return data if isinstance(data, dict) else {}
    except Exception:
        return {}


def emit_json(data: dict[str, Any]) -> None:
    """Write JSON response to stdout."""
    sys.stdout.write(json.dumps(data, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def get_state_path(conversation_id: str | None) -> Path:
    """Return per-conversation state file path in /tmp/agy-ai-review."""
    conv = (conversation_id or "default").strip() or "default"
    digest = hashlib.sha256(conv.encode("utf-8")).hexdigest()[:16]
    return STATE_DIR / f"{digest}.json"


def _ensure_state_dir() -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True, mode=0o700)
    flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        fd = os.open(STATE_DIR, flags)
    except OSError as exc:
        raise PermissionError(f"Unsafe state directory ({exc}): {STATE_DIR}") from exc
    try:
        st = os.fstat(fd)
        if not stat.S_ISDIR(st.st_mode):
            raise PermissionError(f"Unsafe state directory (not a real directory): {STATE_DIR}")
        if st.st_uid != os.getuid():
            raise PermissionError(f"Unsafe state directory owner ({st.st_uid} != {os.getuid()}): {STATE_DIR}")
        if (st.st_mode & 0o077) != 0:
            os.fchmod(fd, 0o700)
            st = os.fstat(fd)
            if (st.st_mode & 0o077) != 0:
                raise PermissionError(f"Unsafe state directory permissions ({oct(st.st_mode)}): {STATE_DIR}")
    finally:
        os.close(fd)


def default_state() -> dict[str, Any]:
    return {
        "baseline_head": "",
        "baseline_diff_hash": "",
        "last_reviewed_diff_hash": "",
        "last_verdict": "",
        "last_reason": "",
        "last_model_tag": "",
        "turn_rounds": 0,
        "edited_files": [],
    }


def load_state(conversation_id: str | None) -> dict[str, Any]:
    try:
        _ensure_state_dir()
    except OSError as exc:
        print(f"[ai-code-review] State dir check failed: {exc}", file=sys.stderr)
        return default_state()
    path = get_state_path(conversation_id)
    if not path.is_file():
        return default_state()
    try:
        with path.open("r", encoding="utf-8") as fh:
            data = json.load(fh)
        if not isinstance(data, dict):
            return default_state()
        state = default_state()
        state.update(data)
        if not isinstance(state.get("edited_files"), list):
            state["edited_files"] = []
        state["turn_rounds"] = _safe_int(state.get("turn_rounds"), 0, 0)
        return state
    except Exception:
        return default_state()


def save_state(conversation_id: str | None, state: dict[str, Any]) -> None:
    _ensure_state_dir()
    path = get_state_path(conversation_id)
    fd, tmp_name = tempfile.mkstemp(prefix=".state-", suffix=".tmp", dir=str(STATE_DIR))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(state, fh, indent=2, ensure_ascii=False)
        os.replace(tmp_name, path)
    except Exception:
        try:
            os.unlink(tmp_name)
        except OSError:
            pass
        raise


def resolve_cwd(payload: dict[str, Any]) -> Path:
    """Determine workspace working directory from hook payload."""
    cwd_str = payload.get("cwd")
    if isinstance(cwd_str, str) and cwd_str.strip():
        p = Path(cwd_str).expanduser()
        if p.exists():
            return p.resolve()
    for list_key in ("workspacePaths", "workspace_roots"):
        ws_paths = payload.get(list_key)
        if isinstance(ws_paths, list):
            for item in ws_paths:
                if isinstance(item, str) and item.strip():
                    p = Path(item).expanduser()
                    if p.exists():
                        return p.resolve()
    proj_dir = os.environ.get("CLAUDE_PROJECT_DIR", "").strip()
    if proj_dir:
        p = Path(proj_dir).expanduser()
        if p.exists():
            return p.resolve()
    return Path.cwd().resolve()


def is_ignored(rel_path: str, patterns: list[str]) -> bool:
    """Check whether a relative path matches any ignore pattern."""
    norm = rel_path.replace("\\", "/")
    while norm.startswith("./"):
        norm = norm[2:]
    norm = norm.lstrip("/")
    if not norm:
        return True
    name = Path(norm).name
    for pat in patterns:
        if fnmatch.fnmatch(norm, pat) or fnmatch.fnmatch(name, pat):
            return True
        if pat.endswith("/*") and norm.startswith(pat[:-1]):
            return True
    return False


def is_subpath(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def run_git(cwd: Path, args: list[str]) -> subprocess.CompletedProcess[str] | None:
    try:
        env = dict(os.environ)
        env["GIT_LITERAL_PATHSPECS"] = "1"
        return subprocess.run(
            ["git", *args],
            cwd=str(cwd),
            env=env,
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
    except Exception:
        return None


def get_git_root(cwd: Path) -> Path | None:
    res = run_git(cwd, ["rev-parse", "--show-toplevel"])
    if res and res.returncode == 0 and res.stdout.strip():
        return Path(res.stdout.strip()).resolve()
    return None


def get_git_head(git_root: Path) -> str:
    res = run_git(git_root, ["rev-parse", "--verify", "HEAD"])
    if res and res.returncode == 0:
        return res.stdout.strip()
    return ""


def _parse_porcelain_z(porcelain_out: str) -> tuple[list[str], list[str]]:
    """Return (tracked_changed_paths, untracked_paths) from `git status --porcelain=v1 -z`."""
    tracked: list[str] = []
    untracked: list[str] = []
    tokens = porcelain_out.split("\x00")
    idx = 0
    while idx < len(tokens):
        entry = tokens[idx]
        idx += 1
        if len(entry) < 4:
            continue
        status = entry[:2]
        path_part = entry[3:]
        if "R" in status or "C" in status:
            # Rename/copy in `-z` is followed by the original source path token.
            if idx < len(tokens):
                idx += 1
        if status == "??":
            untracked.append(path_part)
        else:
            tracked.append(path_part)
    return tracked, untracked


def _read_text_preview_and_hash(
    file_path: Path,
    root: Path,
    max_preview_bytes: int = 20000,
) -> tuple[str, str]:
    """Stream entire file to compute SHA-256 while keeping only a bounded text preview."""
    if not is_subpath(file_path, root):
        return "<symlink outside workspace omitted>", "symlink-outside-workspace"

    hasher = hashlib.sha256()
    preview_chunks: list[bytes] = []
    preview_len = 0
    try:
        with file_path.open("rb") as fh:
            while True:
                chunk = fh.read(65536)
                if not chunk:
                    break
                hasher.update(chunk)
                if preview_len <= max_preview_bytes:
                    need = (max_preview_bytes + 1) - preview_len
                    preview_chunks.append(chunk[:need])
                    preview_len += len(chunk[:need])
    except OSError as exc:
        return f"<unreadable: {exc}>", f"unreadable:{exc}"

    raw_preview = b"".join(preview_chunks)
    if b"\x00" in raw_preview[:4096]:
        return "<binary file omitted>", hasher.hexdigest()

    truncated = len(raw_preview) > max_preview_bytes
    text = raw_preview[:max_preview_bytes].decode("utf-8", errors="replace")
    if truncated:
        text += "\n... [file content truncated]"
    return text, hasher.hexdigest()


def collect_git_snapshot(
    git_root: Path, ignore_patterns: list[str], baseline_head: str = ""
) -> tuple[str, str, str, str, str]:
    """Collect (summary, diff_bundle, diff_hash, current_head, dirty_hash) from one snapshot."""
    status_res = run_git(git_root, ["status", "--porcelain=v1", "-z", "--untracked-files=all"])
    if not status_res or status_res.returncode != 0:
        return "", "", "", "", ""

    tracked, untracked = _parse_porcelain_z(status_res.stdout)
    kept_tracked = [p for p in tracked if not is_ignored(p, ignore_patterns)]
    kept_untracked = [p for p in untracked if not is_ignored(p, ignore_patterns)]

    current_head = get_git_head(git_root) if baseline_head else ""
    kept_committed: list[str] = []
    if baseline_head and current_head and current_head != baseline_head:
        committed_names = run_git(
            git_root, ["diff", "--name-only", "-z", f"{baseline_head}..HEAD"]
        )
        if committed_names and committed_names.returncode == 0:
            kept_committed = [
                p
                for p in committed_names.stdout.split("\x00")
                if p and not is_ignored(p, ignore_patterns)
            ]

    if not kept_tracked and not kept_untracked and not kept_committed:
        return "", "", "", current_head, ""

    parts: list[str] = []
    dirty_parts: list[str] = []
    summary_lines: list[str] = []
    untracked_hashes: dict[str, str] = {}

    if kept_committed:
        c_stat = run_git(
            git_root, ["diff", "--stat", f"{baseline_head}..HEAD", "--", *kept_committed]
        )
        if c_stat and c_stat.stdout.strip():
            summary_lines.append(
                f"Committed ({baseline_head[:7]}..{current_head[:7]}):\n{c_stat.stdout.strip()}"
            )
        c_diff = run_git(
            git_root, ["diff", f"{baseline_head}..HEAD", "--", *kept_committed]
        )
        if c_diff and c_diff.returncode == 0 and c_diff.stdout.strip():
            parts.append(
                f"### Committed Changes ({baseline_head[:7]}..{current_head[:7]})\n"
                f"```diff\n{c_diff.stdout.strip()}\n```"
            )

    if kept_tracked:
        stat_res = run_git(git_root, ["diff", "--stat", "HEAD", "--", *kept_tracked])
        if not stat_res or stat_res.returncode != 0:
            stat_res = run_git(git_root, ["diff", "--stat", "--", *kept_tracked])
        if stat_res and stat_res.stdout.strip():
            summary_lines.append(stat_res.stdout.strip())

        staged_res = run_git(git_root, ["diff", "--staged", "--", *kept_tracked])
        if staged_res and staged_res.returncode == 0 and staged_res.stdout.strip():
            block = "### Staged Changes\n```diff\n" + staged_res.stdout.strip() + "\n```"
            parts.append(block)
            dirty_parts.append(block)

        unstaged_res = run_git(git_root, ["diff", "--", *kept_tracked])
        if unstaged_res and unstaged_res.returncode == 0 and unstaged_res.stdout.strip():
            block = "### Unstaged Changes\n```diff\n" + unstaged_res.stdout.strip() + "\n```"
            parts.append(block)
            dirty_parts.append(block)

    if kept_untracked:
        summary_lines.append("Untracked files:\n" + "\n".join(f"  + {p}" for p in kept_untracked))
        for rel in kept_untracked:
            target = git_root / rel
            if target.is_file():
                preview, file_hash = _read_text_preview_and_hash(target, git_root)
                untracked_hashes[rel] = file_hash
                block = f"### New File: {rel}\n```\n{preview}\n```"
                parts.append(block)
                dirty_parts.append(block)

    bundle = "\n\n".join(parts).strip()
    if not bundle:
        return "", "", "", current_head, ""

    dirty_bundle = "\n\n".join(dirty_parts).strip()
    dirty_hash = ""
    if dirty_bundle:
        dirty_input = json.dumps(
            {
                "committed_head": "",
                "committed": [],
                "tracked": sorted(kept_tracked),
                "untracked": untracked_hashes,
                "bundle": dirty_bundle,
            },
            sort_keys=True,
        )
        dirty_hash = hashlib.sha256(dirty_input.encode("utf-8")).hexdigest()

    summary = "\n".join(summary_lines).strip()
    hash_input = json.dumps(
        {
            "committed_head": current_head if kept_committed else "",
            "committed": sorted(kept_committed),
            "tracked": sorted(kept_tracked),
            "untracked": untracked_hashes,
            "bundle": bundle,
        },
        sort_keys=True,
    )
    diff_hash = hashlib.sha256(hash_input.encode("utf-8")).hexdigest()
    return summary, bundle, diff_hash, current_head, dirty_hash


def collect_git_changes(
    git_root: Path, ignore_patterns: list[str], baseline_head: str = ""
) -> tuple[str, str, str]:
    """Collect (summary, diff_bundle, diff_hash) for non-ignored git changes."""
    summary, bundle, diff_hash, _, _ = collect_git_snapshot(
        git_root, ignore_patterns, baseline_head=baseline_head
    )
    return summary, bundle, diff_hash


def collect_tracked_file_changes(
    cwd: Path, edited_files: list[str], ignore_patterns: list[str]
) -> tuple[str, str, str]:
    """Fallback change collector when workspace is not a git repository."""
    kept = [p for p in edited_files if not is_ignored(p, ignore_patterns)]
    if not kept:
        return "", "", ""

    parts: list[str] = []
    file_hashes: dict[str, str] = {}
    for rel in sorted(set(kept)):
        raw_p = Path(rel)
        target = raw_p if raw_p.is_absolute() else (cwd / raw_p)
        root_for_check = raw_p.parent if raw_p.is_absolute() else cwd
        if target.is_file() or target.is_symlink():
            preview, file_hash = _read_text_preview_and_hash(target, root_for_check)
            file_hashes[rel] = file_hash
            parts.append(f"### Modified File: {rel}\n```\n{preview}\n```")
        elif not target.exists():
            file_hashes[rel] = "deleted"
            parts.append(f"### Deleted File: {rel}\n<file deleted>")

    bundle = "\n\n".join(parts).strip()
    if not bundle:
        return "", "", ""
    summary = "Edited files:\n" + "\n".join(f"  * {p}" for p in sorted(set(kept)))
    hash_input = json.dumps({"files": file_hashes}, sort_keys=True)
    diff_hash = hashlib.sha256(hash_input.encode("utf-8")).hexdigest()
    return summary, bundle, diff_hash


def build_augmented_env() -> dict[str, str]:
    """Build subprocess environment with recursion guard and common CLI paths."""
    env = dict(os.environ)
    env["AI_REVIEW_HOOK_ACTIVE"] = "1"
    extra_dirs = [
        str(Path.home() / ".local" / "bin"),
        "/opt/homebrew/bin",
        "/usr/local/bin",
    ]
    current_path = env.get("PATH", "")
    path_parts = [p for p in current_path.split(os.pathsep) if p]
    for d in reversed(extra_dirs):
        if d not in path_parts:
            path_parts.insert(0, d)
    env["PATH"] = os.pathsep.join(path_parts)
    return env


def build_review_prompt(
    last_user_input: str,
    summary: str,
    diff_bundle: str,
    config: dict[str, Any],
) -> str:
    """Construct strict, concise code review prompt for external AI model."""
    max_chars = int(config.get("max_diff_chars", 60000))
    if len(diff_bundle) > max_chars:
        diff_text = (
            diff_bundle[:max_chars]
            + f"\n\n... [TRUNCATED: diff exceeded max_diff_chars={max_chars}]"
        )
    else:
        diff_text = diff_bundle

    task_ctx = (last_user_input or "").strip()[:1500] or "(not provided)"
    extra = str(config.get("extra_instructions", "")).strip()
    extra_block = f"\nProject-specific guidelines:\n{extra}\n" if extra else ""

    include_nits = bool(config.get("include_nits", True))
    if include_nits:
        rules_block = (
            "Review Instructions:\n"
            "1. Your VERY FIRST non-empty line MUST be exactly one of:\n"
            "   VERDICT: PASS\n"
            "   VERDICT: NEEDS_FIX\n"
            "2. Use `VERDICT: NEEDS_FIX` whenever ANY actionable finding exists — including "
            "CRITICAL, HIGH, MEDIUM, LOW, and NIT items (bugs, broken logic, security vulnerabilities, "
            "panics/crashes, broken edge cases, violations of project guidelines, naming, readability, "
            "cleanliness, idioms, minor simplifications, missing edge-case tests, or docs/comments).\n"
            "3. Return `VERDICT: PASS` ONLY when there are zero remaining issues or nits.\n"
            "4. Stability rule: avoid contradictory or oscillating nits across rounds; only raise "
            "concrete, unambiguous improvements.\n"
            "5. If `VERDICT: NEEDS_FIX`, list each finding concisely as:\n"
            "   - [CRITICAL|HIGH|MEDIUM|LOW|NIT] path:line — problem description and concrete fix.\n"
            "6. If `VERDICT: PASS`, briefly confirm what looks good (1-3 bullets max).\n\n"
        )
    else:
        rules_block = (
            "Review Instructions:\n"
            "1. Your VERY FIRST non-empty line MUST be exactly one of:\n"
            "   VERDICT: PASS\n"
            "   VERDICT: NEEDS_FIX\n"
            "2. Use `VERDICT: NEEDS_FIX` ONLY for concrete, actionable defects: bugs, broken logic, "
            "security vulnerabilities (SQL injection, XSS, missing auth/permission checks, SSRF, secrets), "
            "panics/crashes (e.g. `.unwrap()` in production paths), broken edge cases, or clear violations "
            "of the project guidelines.\n"
            "3. Do NOT fail for style nitpicks, minor naming preferences, or speculative refactors.\n"
            "4. If `VERDICT: NEEDS_FIX`, list each issue concisely as:\n"
            "   - [SEVERITY] path:line — problem description and concrete fix.\n"
            "5. If `VERDICT: PASS`, briefly confirm what looks good (1-3 bullets max).\n\n"
        )

    return (
        "You are an independent senior software engineer performing a focused code review "
        "on changes just written by another AI coding agent.\n\n"
        f"User task context:\n{task_ctx}\n"
        f"{extra_block}\n"
        f"{rules_block}"
        f"Changed Files Summary:\n{summary or '(none)'}\n\n"
        f"Changes:\n{diff_text}\n"
    )


def run_reviewer(
    prompt: str, cwd: Path, config: dict[str, Any]
) -> tuple[bool, str, str]:
    """Invoke configured AI reviewer CLI. Returns (ok, output_text, error_msg)."""
    provider = config["provider"]
    model = config["model"]
    timeout = int(config["timeout_seconds"])
    env = build_augmented_env()
    search_path = env["PATH"]

    if provider == "claude":
        binary = shutil.which("claude", path=search_path)
        if not binary:
            return False, "", "claude CLI not found in PATH"
        cmd = [
            binary,
            "-p",
            "--no-session-persistence",
            "--output-format",
            "text",
            "--tools",
            "",
        ]
        if model:
            cmd.extend(["--model", model])
        return _exec_cli(cmd, prompt, cwd, env, timeout)

    if provider == "codex":
        binary = shutil.which("codex", path=search_path)
        if not binary:
            return False, "", "codex CLI not found in PATH"
        fd, out_file = tempfile.mkstemp(prefix="codex-review-", suffix=".txt")
        os.close(fd)
        try:
            cmd = [
                binary,
                "exec",
                "--ephemeral",
                "--sandbox",
                "read-only",
                "--skip-git-repo-check",
                "-o",
                out_file,
                "-",
            ]
            if model:
                cmd.extend(["-m", model])
            ok, stdout_text, err_msg = _exec_cli(cmd, prompt, cwd, env, timeout)
            file_text = ""
            try:
                file_text = Path(out_file).read_text(encoding="utf-8").strip()
            except OSError:
                pass
            output = file_text or stdout_text
            if not ok:
                return False, output, err_msg
            return True, output, ""
        finally:
            try:
                os.unlink(out_file)
            except OSError:
                pass

    if provider == "custom":
        raw_cmd = config.get("custom_command", [])
        if not raw_cmd or not all(isinstance(x, str) and x for x in raw_cmd):
            return False, "", "custom_command must be a non-empty list of strings"
        cmd = [part.replace("{model}", model) for part in raw_cmd]
        return _exec_cli(cmd, prompt, cwd, env, timeout)

    return False, "", f"Unsupported provider: {provider}"


def _exec_cli(
    cmd: list[str],
    prompt: str,
    cwd: Path,
    env: dict[str, str],
    timeout: int,
) -> tuple[bool, str, str]:
    try:
        proc = subprocess.run(
            cmd,
            input=prompt,
            cwd=str(cwd),
            env=env,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return False, "", f"Reviewer command timed out after {timeout}s"
    except Exception as exc:
        return False, "", f"Failed to execute reviewer command: {exc}"

    stdout_text = (proc.stdout or "").strip()
    stderr_text = (proc.stderr or "").strip()
    if proc.returncode != 0:
        detail = stderr_text or stdout_text or f"exit status {proc.returncode}"
        return False, stdout_text, f"Reviewer exited with {proc.returncode}: {detail}"
    if not stdout_text:
        return False, "", "Reviewer returned empty output"
    return True, stdout_text, ""


def parse_verdict(review_output: str) -> str:
    """Return 'PASS', 'NEEDS_FIX', or 'UNKNOWN'."""
    match = VERDICT_RE.search(review_output or "")
    if not match:
        return "UNKNOWN"
    return match.group(1).upper()


def write_artifact_report(
    payload: dict[str, Any],
    provider: str,
    model: str,
    round_num: int,
    max_rounds: int,
    verdict: str,
    body: str,
    session_key: str = "",
) -> None:
    """Persist markdown review report to artifactDirectoryPath or /tmp fallback."""
    art_dir = payload.get("artifactDirectoryPath")
    try:
        if isinstance(art_dir, str) and art_dir.strip():
            target_dir = Path(art_dir).expanduser()
            target_dir.mkdir(parents=True, exist_ok=True)
            report_path = target_dir / "ai-code-review.md"
        else:
            _ensure_state_dir()
            target_dir = STATE_DIR
            digest = hashlib.sha256((session_key or "default").encode("utf-8")).hexdigest()[:16]
            report_path = STATE_DIR / f"{digest}-report.md"

        ts = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")
        model_label = model or "default"
        content = (
            f"# AI Code Review Report\n\n"
            f"- **Timestamp:** {ts}\n"
            f"- **Provider:** `{provider}` (model: `{model_label}`)\n"
            f"- **Round:** {round_num}/{max_rounds}\n"
            f"- **Verdict:** `{verdict}`\n\n"
            f"---\n\n"
            f"{body.strip()}\n"
        )
        fd, tmp_name = tempfile.mkstemp(
            prefix=".report-", suffix=".tmp", dir=str(target_dir)
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as fh:
                fh.write(content)
            os.replace(tmp_name, report_path)
        except Exception:
            try:
                os.unlink(tmp_name)
            except OSError:
                pass
            raise
    except Exception as exc:
        print(f"[ai-code-review] Failed to write artifact report: {exc}", file=sys.stderr)


def _extract_candidate_paths(payload: dict[str, Any]) -> list[str]:
    """Extract edited file paths across Antigravity, Codex, Claude Code, and Cursor payloads."""
    candidates: list[str] = []
    for top_key in ("file_path", "path"):
        val = payload.get(top_key)
        if isinstance(val, str) and val.strip():
            candidates.append(val.strip())

    arg_dicts: list[dict[str, Any]] = []
    tool_call = payload.get("toolCall")
    if isinstance(tool_call, dict) and isinstance(tool_call.get("args"), dict):
        arg_dicts.append(tool_call["args"])

    tool_input = payload.get("tool_input")
    if isinstance(tool_input, dict):
        arg_dicts.append(tool_input)
    elif isinstance(tool_input, str) and tool_input.strip():
        candidates.extend(
            m.strip() for m in APPLY_PATCH_FILE_RE.findall(tool_input) if m.strip()
        )

    for args in arg_dicts:
        for key in ("Path", "TargetFile", "path", "file_path", "notebook_path", "target_file"):
            val = args.get(key)
            if isinstance(val, str) and val.strip():
                candidates.append(val.strip())
                break
        for patch_key in ("input", "patch", "raw_input"):
            patch_val = args.get(patch_key)
            if isinstance(patch_val, str) and patch_val.strip():
                candidates.extend(
                    m.strip() for m in APPLY_PATCH_FILE_RE.findall(patch_val) if m.strip()
                )
    return candidates


def _is_continuation_active(payload: dict[str, Any], runtime: str) -> bool:
    if runtime == "antigravity":
        return bool(payload.get("continuationActive", False))
    if runtime == "cursor":
        try:
            return int(payload.get("loop_count", 0)) > 0
        except (TypeError, ValueError):
            return False
    return bool(payload.get("stop_hook_active", False))


def _should_skip_stop(payload: dict[str, Any], runtime: str) -> bool:
    if runtime == "antigravity":
        termination_reason = str(payload.get("terminationReason", "")).strip()
        if termination_reason and termination_reason != "NO_TOOL_CALL":
            return True
        if payload.get("fullyIdle") is False:
            return True
    elif runtime == "cursor":
        status = str(payload.get("status", "")).strip().lower()
        if status and status != "completed":
            return True
    return False


def _extract_user_context(payload: dict[str, Any]) -> str:
    for key in ("lastUserInput", "prompt", "user_prompt", "last_assistant_message"):
        val = payload.get(key)
        if isinstance(val, str) and val.strip():
            return val.strip()
    return ""


def handle_session_start(
    payload: dict[str, Any], config: dict[str, Any], runtime: str = "antigravity"
) -> dict[str, Any]:
    cwd = resolve_cwd(payload)
    session_key = extract_session_id(payload, runtime)
    git_root = get_git_root(cwd)
    baseline_hash = ""
    baseline_head = ""
    if git_root:
        baseline_head = get_git_head(git_root)
        _, _, baseline_hash = collect_git_changes(git_root, config["ignore_patterns"])

    state = default_state()
    state["baseline_head"] = baseline_head
    state["baseline_diff_hash"] = baseline_hash
    save_state(session_key, state)
    return {}


def handle_track(
    payload: dict[str, Any], config: dict[str, Any], runtime: str = "antigravity"
) -> dict[str, Any]:
    if payload.get("error"):
        return {}

    candidates = _extract_candidate_paths(payload)
    if not candidates:
        return {}

    cwd = resolve_cwd(payload)
    git_root = get_git_root(cwd)
    art_dir = payload.get("artifactDirectoryPath")
    art_path = Path(art_dir).expanduser() if isinstance(art_dir, str) and art_dir.strip() else None

    ws_roots: list[Path] = []
    if git_root:
        ws_roots.append(git_root)
    ws_roots.append(cwd)
    for list_key in ("workspacePaths", "workspace_roots"):
        if isinstance(payload.get(list_key), list):
            for p in payload[list_key]:
                if isinstance(p, str) and p.strip():
                    ws_roots.append(Path(p).expanduser().resolve())

    session_key = extract_session_id(payload, runtime)
    state = load_state(session_key)
    changed = False

    for raw_path in candidates:
        candidate = Path(raw_path).expanduser()
        if not candidate.is_absolute():
            candidate = (cwd / candidate).resolve()
        else:
            candidate = candidate.resolve()

        if art_path and is_subpath(candidate, art_path):
            continue

        matched_root = None
        for root in ws_roots:
            if is_subpath(candidate, root):
                matched_root = root
                break
        if matched_root is None:
            continue

        rel_path = candidate.relative_to(matched_root).as_posix()
        if is_ignored(rel_path, config["ignore_patterns"]):
            continue

        stored_ref = (
            rel_path
            if (git_root is not None or matched_root == cwd)
            else str(candidate)
        )
        if stored_ref not in state["edited_files"]:
            state["edited_files"].append(stored_ref)
            changed = True

    if changed:
        save_state(session_key, state)
    return {}


def _sync_baseline_after_review(
    state: dict[str, Any],
    git_root: Path | None,
    reviewed_head: str,
    reviewed_dirty_hash: str,
    current_hash: str,
) -> None:
    """Advance baseline strictly to the exact snapshot that was reviewed (zero TOCTOU)."""
    if not git_root:
        state["last_reviewed_diff_hash"] = current_hash
        state["baseline_diff_hash"] = current_hash
        return
    if reviewed_head:
        state["baseline_head"] = reviewed_head
    state["baseline_diff_hash"] = reviewed_dirty_hash
    state["last_reviewed_diff_hash"] = reviewed_dirty_hash


def handle_review(
    payload: dict[str, Any], config: dict[str, Any], runtime: str = "antigravity"
) -> dict[str, Any]:
    if _should_skip_stop(payload, runtime):
        return allow_response(runtime, "review")

    session_key = extract_session_id(payload, runtime)
    state = load_state(session_key)
    if not _is_continuation_active(payload, runtime):
        state["turn_rounds"] = 0

    cwd = resolve_cwd(payload)
    git_root = get_git_root(cwd)
    reviewed_head = ""
    reviewed_dirty_hash = ""
    if git_root:
        summary, bundle, current_hash, reviewed_head, reviewed_dirty_hash = (
            collect_git_snapshot(
                git_root,
                config["ignore_patterns"],
                baseline_head=state.get("baseline_head", ""),
            )
        )
    else:
        summary, bundle, current_hash = collect_tracked_file_changes(
            cwd, state["edited_files"], config["ignore_patterns"]
        )

    has_tracked_edits = len(state["edited_files"]) > 0
    has_unfixed_needs_fix = (
        bool(current_hash)
        and current_hash == state["last_reviewed_diff_hash"]
        and state.get("last_verdict") == "NEEDS_FIX"
    )
    has_new_git_diff = (
        bool(current_hash)
        and current_hash != state["baseline_diff_hash"]
        and current_hash != state["last_reviewed_diff_hash"]
    )

    if (
        not has_tracked_edits and not has_new_git_diff and not has_unfixed_needs_fix
    ) or not current_hash:
        state["edited_files"] = []
        save_state(session_key, state)
        return allow_response(runtime, "review")

    max_rounds = int(config["max_review_rounds"])
    reviewer_cfg = resolve_reviewer_config(config, runtime)
    follow_up = (
        "Please address EVERY finding above (including LOW and NIT items), re-run verification checks, and finish."
        if reviewer_cfg.get("include_nits", True)
        else "Please address the actionable issues above, re-run verification checks, and finish."
    )

    if current_hash == state["last_reviewed_diff_hash"]:
        if state.get("last_verdict") == "NEEDS_FIX":
            if state["turn_rounds"] >= max_rounds:
                _sync_baseline_after_review(
                    state, git_root, reviewed_head, reviewed_dirty_hash, current_hash
                )
                state["last_verdict"] = "CAPPED"
                state["last_reason"] = ""
                state["last_model_tag"] = ""
                state["edited_files"] = []
                save_state(session_key, state)
                return allow_response(runtime, "review")
            state["turn_rounds"] += 1
            if git_root:
                state["edited_files"] = []
            save_state(session_key, state)
            prev_findings = state.get("last_reason", "").strip()
            m_tag = state.get("last_model_tag") or reviewer_cfg["provider"]
            reblock_reason = (
                f"[AI Code Review ({m_tag}, round {state['turn_rounds']}/{max_rounds}) — NEEDS_FIX (diff unchanged)]\n\n"
                f"{prev_findings}\n\n"
                f"{follow_up}"
            ).strip()
            return block_response(runtime, reblock_reason)
        state["edited_files"] = []
        save_state(session_key, state)
        return allow_response(runtime, "review")

    if state["turn_rounds"] >= max_rounds:
        _sync_baseline_after_review(
            state, git_root, reviewed_head, reviewed_dirty_hash, current_hash
        )
        state["last_verdict"] = "CAPPED"
        state["last_reason"] = ""
        state["last_model_tag"] = ""
        state["edited_files"] = []
        save_state(session_key, state)
        return allow_response(runtime, "review")

    prompt = build_review_prompt(
        _extract_user_context(payload),
        summary,
        bundle,
        reviewer_cfg,
    )
    ok, review_output, err_msg = run_reviewer(prompt, git_root or cwd, reviewer_cfg)
    round_num = state["turn_rounds"] + 1
    provider = reviewer_cfg["provider"]
    model = reviewer_cfg["model"]

    if not ok:
        print(f"[ai-code-review] Fail-open ({provider}): {err_msg}", file=sys.stderr)
        write_artifact_report(
            payload,
            provider,
            model,
            round_num,
            max_rounds,
            "ERROR (fail-open)",
            f"Reviewer failed to run: {err_msg}\n",
            session_key=session_key,
        )
        if git_root:
            state["edited_files"] = []
        save_state(session_key, state)
        return allow_response(runtime, "review")

    verdict = parse_verdict(review_output)
    write_artifact_report(
        payload,
        provider,
        model,
        round_num,
        max_rounds,
        verdict,
        review_output,
        session_key=session_key,
    )

    if verdict == "NEEDS_FIX":
        model_tag = f"{provider}/{model}" if model else provider
        findings_body = review_output.strip()
        reason = (
            f"[AI Code Review ({model_tag}, round {round_num}/{max_rounds}) — NEEDS_FIX]\n\n"
            f"{findings_body}\n\n"
            f"{follow_up}"
        )
        state["turn_rounds"] = round_num
        state["last_reviewed_diff_hash"] = current_hash
        state["last_verdict"] = "NEEDS_FIX"
        state["last_reason"] = findings_body
        state["last_model_tag"] = model_tag
        if git_root:
            state["edited_files"] = []
        save_state(session_key, state)
        return block_response(runtime, reason)

    if verdict == "UNKNOWN":
        print(
            f"[ai-code-review] Reviewer ({provider}) output missing VERDICT line (fail-open)",
            file=sys.stderr,
        )
        if git_root:
            state["edited_files"] = []
        save_state(session_key, state)
        return allow_response(runtime, "review")

    _sync_baseline_after_review(
        state, git_root, reviewed_head, reviewed_dirty_hash, current_hash
    )
    state["last_verdict"] = "PASS"
    state["last_reason"] = ""
    state["last_model_tag"] = ""
    state["edited_files"] = []
    save_state(session_key, state)
    return allow_response(runtime, "review")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Unified AI code review lifecycle hook")
    parser.add_argument(
        "--runtime",
        choices=["auto", "antigravity", "codex", "claude", "cursor"],
        default="auto",
        help="Calling agent runtime (default: auto-detect from stdin payload)",
    )
    parser.add_argument("action", choices=["session-start", "track", "review"])
    args = parser.parse_args(argv)

    payload = read_stdin_json()
    runtime = detect_runtime(payload, args.runtime)
    config = load_config()

    if _truthy_env("AI_REVIEW_HOOK_ACTIVE") or not config.get("enabled", True):
        emit_json(allow_response(runtime, args.action))
        return 0

    try:
        if args.action == "session-start":
            result = handle_session_start(payload, config, runtime=runtime)
        elif args.action == "track":
            result = handle_track(payload, config, runtime=runtime)
        else:
            result = handle_review(payload, config, runtime=runtime)
    except Exception as exc:
        print(f"[ai-code-review] Unexpected error (fail-open): {exc}", file=sys.stderr)
        result = allow_response(runtime, args.action)

    emit_json(result)
    return 0


if __name__ == "__main__":
    sys.exit(main())
