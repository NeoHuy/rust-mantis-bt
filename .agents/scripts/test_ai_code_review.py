#!/usr/bin/env python3
"""Unit tests for .agents/scripts/ai_code_review.py."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

import ai_code_review as hook  # noqa: E402


def _init_git_repo(repo_dir: Path) -> None:
    subprocess.run(["git", "init"], cwd=repo_dir, check=True, capture_output=True)
    subprocess.run(
        ["git", "config", "user.email", "test@example.com"],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "config", "user.name", "Test User"],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )
    (repo_dir / "src").mkdir(exist_ok=True)
    (repo_dir / "src" / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    subprocess.run(["git", "add", "."], cwd=repo_dir, check=True, capture_output=True)
    subprocess.run(
        ["git", "commit", "-m", "init"],
        cwd=repo_dir,
        check=True,
        capture_output=True,
    )


class AiCodeReviewHookTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        _init_git_repo(self.repo)
        self.artifacts = self.root / "artifacts"
        self.artifacts.mkdir()
        self.state_dir = self.root / "state"
        self.state_dir.mkdir()
        self.orig_state_dir = hook.STATE_DIR
        hook.STATE_DIR = self.state_dir
        self.conv_id = "test-conv-123"
        self.cfg = hook.load_config(Path("/nonexistent/config.json"))

    def tearDown(self) -> None:
        hook.STATE_DIR = self.orig_state_dir
        self.tmp.cleanup()

    def _base_payload(self, **extra: object) -> dict[str, object]:
        payload: dict[str, object] = {
            "conversationId": self.conv_id,
            "workspacePaths": [str(self.repo)],
            "artifactDirectoryPath": str(self.artifacts),
            "cwd": str(self.repo),
            "lastUserInput": "Implement feature X",
        }
        payload.update(extra)
        return payload

    def test_session_start_records_baseline_and_allows_clean_stop(self) -> None:
        # Pre-existing dirty change before session starts
        (self.repo / "src" / "main.rs").write_text("fn main() { /* dirty */ }\n", encoding="utf-8")
        out = hook.handle_session_start(self._base_payload(), self.cfg)
        self.assertEqual(out, {})
        state = hook.load_state(f"antigravity:{self.conv_id}")
        self.assertTrue(state["baseline_diff_hash"])

        # Stop without new edits in this turn -> allow without calling reviewer
        with mock.patch.object(hook, "run_reviewer") as m_rev:
            res = hook.handle_review(
                self._base_payload(terminationReason="NO_TOOL_CALL", fullyIdle=True),
                self.cfg,
            )
            self.assertEqual(res, {"decision": "allow"})
            m_rev.assert_not_called()

    def test_track_filters_artifacts_external_errors_and_ignored(self) -> None:
        # Artifact file -> ignored
        hook.handle_track(
            self._base_payload(
                toolCall={"name": "write_file", "args": {"Path": str(self.artifacts / "plan.md")}}
            ),
            self.cfg,
        )
        # External file -> ignored
        hook.handle_track(
            self._base_payload(
                toolCall={"name": "write_file", "args": {"Path": "/tmp/outside.txt"}}
            ),
            self.cfg,
        )
        # Failed tool call -> ignored
        hook.handle_track(
            self._base_payload(
                error="permission denied",
                toolCall={"name": "edit_file", "args": {"Path": str(self.repo / "src" / "main.rs")}},
            ),
            self.cfg,
        )
        # Ignored pattern -> ignored
        hook.handle_track(
            self._base_payload(
                toolCall={"name": "write_file", "args": {"Path": str(self.repo / "Cargo.lock")}}
            ),
            self.cfg,
        )
        state = hook.load_state(f"antigravity:{self.conv_id}")
        self.assertEqual(state["edited_files"], [])

        # Valid workspace file -> tracked
        hook.handle_track(
            self._base_payload(
                toolCall={"name": "edit_file", "args": {"Path": str(self.repo / "src" / "main.rs")}}
            ),
            self.cfg,
        )
        state = hook.load_state(f"antigravity:{self.conv_id}")
        self.assertEqual(state["edited_files"], ["src/main.rs"])

    def test_review_skips_on_non_normal_termination_or_not_idle_or_disabled(self) -> None:
        (self.repo / "src" / "main.rs").write_text("fn main() { panic!(); }\n", encoding="utf-8")
        with mock.patch.object(hook, "run_reviewer") as m_rev:
            res1 = hook.handle_review(
                self._base_payload(terminationReason="USER_CANCELED", fullyIdle=True),
                self.cfg,
            )
            res2 = hook.handle_review(
                self._base_payload(terminationReason="NO_TOOL_CALL", fullyIdle=False),
                self.cfg,
            )
            self.assertEqual(res1, {"decision": "allow"})
            self.assertEqual(res2, {"decision": "allow"})
            m_rev.assert_not_called()

        with mock.patch.dict(os.environ, {"AI_REVIEW_DISABLED": "1"}):
            cfg = hook.load_config(Path("/nonexistent/config.json"))
            self.assertFalse(cfg["enabled"])

    def test_review_detects_git_diff_without_track_and_blocks_on_needs_fix(self) -> None:
        hook.handle_session_start(self._base_payload(), self.cfg)
        (self.repo / "src" / "main.rs").write_text("fn main() { let _ = x.unwrap(); }\n", encoding="utf-8")

        fake_review = "VERDICT: NEEDS_FIX\n- [HIGH] src/main.rs:1 — Avoid .unwrap() in production.\n"
        with mock.patch.object(hook, "run_reviewer", return_value=(True, fake_review, "")) as m_rev:
            res = hook.handle_review(
                self._base_payload(terminationReason="NO_TOOL_CALL", fullyIdle=True),
                self.cfg,
            )
            m_rev.assert_called_once()

        self.assertEqual(res["decision"], "continue")
        self.assertIn("NEEDS_FIX", res["reason"])
        self.assertIn("Avoid .unwrap()", res["reason"])

        report_file = self.artifacts / "ai-code-review.md"
        self.assertTrue(report_file.is_file())
        self.assertIn("Verdict:** `NEEDS_FIX`", report_file.read_text(encoding="utf-8"))

        state = hook.load_state(f"antigravity:{self.conv_id}")
        self.assertEqual(state["turn_rounds"], 1)

        # Unchanged diff after NEEDS_FIX must re-block without re-running reviewer
        with mock.patch.object(hook, "run_reviewer") as m_rev2:
            res_unfixed = hook.handle_review(
                self._base_payload(
                    terminationReason="NO_TOOL_CALL",
                    fullyIdle=True,
                    continuationActive=True,
                ),
                self.cfg,
            )
            self.assertEqual(res_unfixed["decision"], "continue")
            self.assertIn("diff unchanged", res_unfixed["reason"])
            m_rev2.assert_not_called()

        # Once fixed and PASS, unchanged diff on subsequent stop allows without re-running reviewer
        (self.repo / "src" / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        with mock.patch.object(
            hook, "run_reviewer", return_value=(True, "VERDICT: PASS\n- ok", "")
        ):
            res_pass = hook.handle_review(
                self._base_payload(
                    terminationReason="NO_TOOL_CALL",
                    fullyIdle=True,
                    continuationActive=True,
                ),
                self.cfg,
            )
            self.assertEqual(res_pass, {"decision": "allow"})

        with mock.patch.object(hook, "run_reviewer") as m_rev3:
            res_dup = hook.handle_review(
                self._base_payload(
                    terminationReason="NO_TOOL_CALL",
                    fullyIdle=True,
                    continuationActive=False,
                ),
                self.cfg,
            )
            self.assertEqual(res_dup, {"decision": "allow"})
            m_rev3.assert_not_called()

    def test_dot_path_ignore_deep_merge_and_unknown_verdict(self) -> None:
        self.assertTrue(hook.is_ignored(".git/config", [".git/*"]))
        self.assertTrue(hook.is_ignored("./.git/HEAD", [".git/*"]))
        self.assertFalse(hook.is_ignored("./src/main.rs", [".git/*"]))

        partial_cfg_path = self.root / "partial-config.json"
        partial_cfg_path.write_text(
            json.dumps({"reviewer_by_runtime": {"codex": {"provider": "claude", "model": "opus"}}}),
            encoding="utf-8",
        )
        merged = hook.load_config(partial_cfg_path)
        self.assertEqual(merged["reviewer_by_runtime"]["codex"]["model"], "opus")
        self.assertEqual(merged["reviewer_by_runtime"]["claude"]["provider"], "codex")

        # UNKNOWN verdict fails open and does not mark diff as PASS
        hook.handle_session_start(self._base_payload(), self.cfg)
        (self.repo / "src" / "main.rs").write_text("fn main() { let z = 1; }\n", encoding="utf-8")
        with mock.patch.object(
            hook, "run_reviewer", return_value=(True, "Missing verdict line", "")
        ):
            res_unk = hook.handle_review(
                self._base_payload(terminationReason="NO_TOOL_CALL", fullyIdle=True),
                self.cfg,
            )
            self.assertEqual(res_unk, {"decision": "allow"})
        state = hook.load_state(f"antigravity:{self.conv_id}")
        self.assertEqual(state["last_reviewed_diff_hash"], "")
        self.assertEqual(state["last_verdict"], "")

    def test_review_pass_and_loop_guard_and_turn_reset(self) -> None:
        self.assertEqual(self.cfg["max_review_rounds"], 5)
        self.assertTrue(self.cfg["include_nits"])

        hook.handle_session_start(self._base_payload(), self.cfg)

        # Rounds 1..5: NEEDS_FIX continues up to max_review_rounds=5
        for round_idx in range(1, 6):
            (self.repo / "src" / "main.rs").write_text(
                f"fn main() {{ let a = {round_idx}; }}\n", encoding="utf-8"
            )
            with mock.patch.object(
                hook,
                "run_reviewer",
                return_value=(True, f"VERDICT: NEEDS_FIX\n- [NIT] bug{round_idx}", ""),
            ):
                res = hook.handle_review(
                    self._base_payload(
                        terminationReason="NO_TOOL_CALL",
                        fullyIdle=True,
                        continuationActive=(round_idx > 1),
                    ),
                    self.cfg,
                )
            self.assertEqual(res["decision"], "continue")
            self.assertIn(f"round {round_idx}/5", res["reason"])
            self.assertIn("including LOW and NIT items", res["reason"])

        # Round 6 (max_review_rounds=5 reached): must allow stop without calling reviewer
        (self.repo / "src" / "main.rs").write_text("fn main() { let a = 6; }\n", encoding="utf-8")
        with mock.patch.object(hook, "run_reviewer") as m_rev:
            res_cap = hook.handle_review(
                self._base_payload(
                    terminationReason="NO_TOOL_CALL",
                    fullyIdle=True,
                    continuationActive=True,
                ),
                self.cfg,
            )
        self.assertEqual(res_cap, {"decision": "allow"})
        m_rev.assert_not_called()

        # New user turn (continuationActive=False) resets turn_rounds and PASS allows stop
        (self.repo / "src" / "main.rs").write_text("fn main() { println!(\"ok\"); }\n", encoding="utf-8")
        with mock.patch.object(hook, "run_reviewer", return_value=(True, "VERDICT: PASS\n- Looks good.", "")):
            res_pass = hook.handle_review(
                self._base_payload(
                    terminationReason="NO_TOOL_CALL",
                    fullyIdle=True,
                    continuationActive=False,
                ),
                self.cfg,
            )
        self.assertEqual(res_pass, {"decision": "allow"})
        self.assertEqual(hook.load_state(f"antigravity:{self.conv_id}")["turn_rounds"], 0)

    def test_include_nits_prompt_and_env_override(self) -> None:
        prompt_with_nits = hook.build_review_prompt("task", "1 file", "diff", self.cfg)
        self.assertIn("[CRITICAL|HIGH|MEDIUM|LOW|NIT]", prompt_with_nits)
        self.assertIn("zero remaining issues or nits", prompt_with_nits)

        with mock.patch.dict(os.environ, {"AI_REVIEW_INCLUDE_NITS": "0"}):
            cfg_no_nits = hook.load_config(Path("/nonexistent/config.json"))
            self.assertFalse(cfg_no_nits["include_nits"])
        prompt_no_nits = hook.build_review_prompt("task", "1 file", "diff", cfg_no_nits)
        self.assertIn("Do NOT fail for style nitpicks", prompt_no_nits)

    def test_claude_codex_and_custom_cli_invocations(self) -> None:
        with mock.patch("shutil.which", return_value="/usr/local/bin/claude"), mock.patch.object(
            hook, "_exec_cli", return_value=(True, "VERDICT: PASS", "")
        ) as m_exec:
            cfg = dict(self.cfg, provider="claude", model="sonnet")
            ok, out, _ = hook.run_reviewer("review me", self.repo, cfg)
            self.assertTrue(ok)
            self.assertEqual(out, "VERDICT: PASS")
            cmd = m_exec.call_args[0][0]
            self.assertIn("-p", cmd)
            self.assertIn("--no-session-persistence", cmd)
            self.assertIn("--output-format", cmd)
            self.assertIn("--tools", cmd)
            self.assertIn("--model", cmd)
            self.assertIn("sonnet", cmd)
            self.assertNotIn("--bare", cmd)

        def fake_codex_exec(cmd, prompt, cwd, env, timeout):
            out_idx = cmd.index("-o") + 1
            Path(cmd[out_idx]).write_text("VERDICT: PASS\n- clean", encoding="utf-8")
            return True, "", ""

        with mock.patch("shutil.which", return_value="/usr/local/bin/codex"), mock.patch.object(
            hook, "_exec_cli", side_effect=fake_codex_exec
        ) as m_exec:
            cfg = dict(self.cfg, provider="codex", model="gpt-5.4")
            ok, out, _ = hook.run_reviewer("review me", self.repo, cfg)
            self.assertTrue(ok)
            self.assertIn("VERDICT: PASS", out)
            cmd = m_exec.call_args[0][0]
            self.assertEqual(cmd[1:3], ["exec", "--ephemeral"])
            self.assertIn("--sandbox", cmd)
            self.assertIn("read-only", cmd)
            self.assertIn("-m", cmd)
            self.assertIn("gpt-5.4", cmd)

        with mock.patch.object(
            hook, "_exec_cli", return_value=(True, "VERDICT: PASS", "")
        ) as m_exec:
            cfg = dict(
                self.cfg,
                provider="custom",
                model="my-m",
                custom_command=["my-reviewer", "--model", "{model}"],
            )
            ok, _, _ = hook.run_reviewer("review me", self.repo, cfg)
            self.assertTrue(ok)
            self.assertEqual(m_exec.call_args[0][0], ["my-reviewer", "--model", "my-m"])

    def test_fail_open_on_missing_binary_error_or_timeout(self) -> None:
        hook.handle_session_start(self._base_payload(), self.cfg)
        (self.repo / "src" / "main.rs").write_text("fn main() { let x = 1; }\n", encoding="utf-8")

        with mock.patch("shutil.which", return_value=None):
            res = hook.handle_review(
                self._base_payload(terminationReason="NO_TOOL_CALL", fullyIdle=True),
                self.cfg,
            )
            self.assertEqual(res, {"decision": "allow"})

        with mock.patch.object(hook, "run_reviewer", return_value=(False, "", "timed out")):
            res = hook.handle_review(
                self._base_payload(terminationReason="NO_TOOL_CALL", fullyIdle=True),
                self.cfg,
            )
            self.assertEqual(res, {"decision": "allow"})

    def test_diff_truncation(self) -> None:
        cfg = dict(self.cfg, max_diff_chars=1000)
        long_diff = "a" * 2500
        prompt = hook.build_review_prompt("fix bug", "1 file", long_diff, cfg)
        self.assertIn("[TRUNCATED: diff exceeded max_diff_chars=1000]", prompt)

    def test_recursion_guard_via_main(self) -> None:
        with mock.patch.dict(os.environ, {"AI_REVIEW_HOOK_ACTIVE": "1"}):
            with mock.patch("sys.stdin", new=mock.MagicMock(read=lambda: "{}")):
                with mock.patch.object(hook, "emit_json") as m_emit:
                    rc = hook.main(["--runtime", "antigravity", "review"])
                    self.assertEqual(rc, 0)
                    m_emit.assert_called_once_with({"decision": "allow"})

            for rt in ("codex", "claude", "cursor"):
                with mock.patch("sys.stdin", new=mock.MagicMock(read=lambda: "{}")):
                    with mock.patch.object(hook, "emit_json") as m_emit:
                        rc = hook.main(["--runtime", rt, "review"])
                        self.assertEqual(rc, 0)
                        m_emit.assert_called_once_with({})

    def test_codex_runtime_apply_patch_and_block_decision(self) -> None:
        codex_start = {
            "session_id": "thr_codex_1",
            "cwd": str(self.repo),
        }
        hook.handle_session_start(codex_start, self.cfg, runtime="codex")

        patch_text = "*** Begin Patch\n*** Update File: src/main.rs\n@@\n-fn main() {}\n+fn main() { let _ = 1; }\n*** End Patch\n"
        hook.handle_track(
            {
                "session_id": "thr_codex_1",
                "cwd": str(self.repo),
                "tool_name": "apply_patch",
                "tool_input": {"input": patch_text},
            },
            self.cfg,
            runtime="codex",
        )
        state = hook.load_state("codex:thr_codex_1")
        self.assertEqual(state["edited_files"], ["src/main.rs"])

        (self.repo / "src" / "main.rs").write_text("fn main() { let _ = 1; }\n", encoding="utf-8")
        with mock.patch.object(
            hook,
            "run_reviewer",
            return_value=(True, "VERDICT: NEEDS_FIX\n- [NIT] src/main.rs:1 — unused", ""),
        ) as m_rev:
            res = hook.handle_review(
                {
                    "session_id": "thr_codex_1",
                    "cwd": str(self.repo),
                    "turn_id": "turn_1",
                    "stop_hook_active": False,
                },
                self.cfg,
                runtime="codex",
            )
            self.assertEqual(m_rev.call_args[0][2]["provider"], "claude")
        self.assertEqual(res["decision"], "block")
        self.assertIn("NEEDS_FIX", res["reason"])

        # Fallback report written to /tmp/agy-ai-review/<hash>-report.md
        reports = list(self.state_dir.glob("*-report.md"))
        self.assertEqual(len(reports), 1)
        self.assertIn("Verdict:** `NEEDS_FIX`", reports[0].read_text(encoding="utf-8"))

        # PASS returns {}
        (self.repo / "src" / "main.rs").write_text("fn main() { println!(\"ok\"); }\n", encoding="utf-8")
        with mock.patch.object(
            hook, "run_reviewer", return_value=(True, "VERDICT: PASS\n- clean", "")
        ):
            res_pass = hook.handle_review(
                {
                    "session_id": "thr_codex_1",
                    "cwd": str(self.repo),
                    "stop_hook_active": True,
                },
                self.cfg,
                runtime="codex",
            )
        self.assertEqual(res_pass, {})

    def test_claude_runtime_cross_model_routing_and_env_override(self) -> None:
        claude_cfg = hook.resolve_reviewer_config(self.cfg, "claude")
        self.assertEqual(claude_cfg["provider"], "codex")

        with mock.patch.dict(os.environ, {"AI_REVIEW_PROVIDER": "claude"}):
            overridden_cfg = hook.load_config(Path("/nonexistent/config.json"))
            resolved = hook.resolve_reviewer_config(overridden_cfg, "claude")
            self.assertEqual(resolved["provider"], "claude")

        hook.handle_session_start(
            {"session_id": "cc_1", "cwd": str(self.repo)},
            self.cfg,
            runtime="claude",
        )
        hook.handle_track(
            {
                "session_id": "cc_1",
                "cwd": str(self.repo),
                "tool_input": {"file_path": str(self.repo / "src" / "main.rs")},
            },
            self.cfg,
            runtime="claude",
        )
        (self.repo / "src" / "main.rs").write_text("fn main() { todo!(); }\n", encoding="utf-8")
        with mock.patch.object(
            hook,
            "run_reviewer",
            return_value=(True, "VERDICT: NEEDS_FIX\n- [HIGH] remove todo!()", ""),
        ) as m_rev:
            res = hook.handle_review(
                {"session_id": "cc_1", "cwd": str(self.repo), "stop_hook_active": False},
                self.cfg,
                runtime="claude",
            )
            self.assertEqual(m_rev.call_args[0][2]["provider"], "codex")
        self.assertEqual(res["decision"], "block")
        self.assertIn("codex, round 1/5", res["reason"])

    def test_cursor_runtime_after_file_edit_and_followup_message(self) -> None:
        hook.handle_track(
            {
                "conversation_id": "cur_1",
                "workspace_roots": [str(self.repo)],
                "file_path": str(self.repo / "src" / "main.rs"),
            },
            self.cfg,
            runtime="cursor",
        )
        self.assertEqual(hook.load_state("cursor:cur_1")["edited_files"], ["src/main.rs"])

        # Non-completed status skips review
        (self.repo / "src" / "main.rs").write_text("fn main() { let _ = 42; }\n", encoding="utf-8")
        with mock.patch.object(hook, "run_reviewer") as m_rev:
            res_abort = hook.handle_review(
                {
                    "conversation_id": "cur_1",
                    "workspace_roots": [str(self.repo)],
                    "status": "aborted",
                    "loop_count": 0,
                },
                self.cfg,
                runtime="cursor",
            )
            self.assertEqual(res_abort, {})
            m_rev.assert_not_called()

        # Completed status with NEEDS_FIX -> followup_message
        with mock.patch.object(
            hook,
            "run_reviewer",
            return_value=(True, "VERDICT: NEEDS_FIX\n- [NIT] clean up", ""),
        ):
            res_fix = hook.handle_review(
                {
                    "conversation_id": "cur_1",
                    "workspace_roots": [str(self.repo)],
                    "status": "completed",
                    "loop_count": 0,
                },
                self.cfg,
                runtime="cursor",
            )
        self.assertIn("followup_message", res_fix)
        self.assertIn("NEEDS_FIX", res_fix["followup_message"])

        # PASS -> {}
        (self.repo / "src" / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        with mock.patch.object(
            hook, "run_reviewer", return_value=(True, "VERDICT: PASS\n- ok", "")
        ):
            res_ok = hook.handle_review(
                {
                    "conversation_id": "cur_1",
                    "workspace_roots": [str(self.repo)],
                    "status": "completed",
                    "loop_count": 1,
                },
                self.cfg,
                runtime="cursor",
            )
        self.assertEqual(res_ok, {})


if __name__ == "__main__":
    unittest.main()
