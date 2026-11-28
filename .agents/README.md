# AI Code Review Hook (Multi-Agent: Antigravity, Codex, Claude Code, Cursor)

Kiến trúc **1 Core Engine + Thin Manifests**: toàn bộ logic review, prompt, `max_review_rounds`, `include_nits` nằm tập trung tại `.agents/scripts/ai_code_review.py` và `.agents/ai-review-config.json`. Các AI Agent khác nhau chỉ cần một manifest mỏng trỏ về cùng script này.

## Các AI Agent được hỗ trợ

| AI Agent | Manifest | Cách `Stop` yêu cầu sửa tiếp | Reviewer mặc định (`reviewer_by_runtime`) |
| :--- | :--- | :--- | :--- |
| **Antigravity** | `.agents/hooks.json` | `{"decision": "continue", "reason": "..."}` | `claude` |
| **Codex CLI** | `.codex/hooks.json` | `{"decision": "block", "reason": "..."}` | `claude` |
| **Claude Code** | `.claude/settings.json` | `{"decision": "block", "reason": "..."}` | `codex` *(cross-model)* |
| **Cursor** | `.cursor/hooks.json` | `{"followup_message": "..."}` | `claude` |

> **Lưu ý cho Codex CLI:** Lần đầu sử dụng hook trên workspace mới, mở Codex CLI và chạy `/hooks` để trust file `.codex/hooks.json`.

## Cách hoạt động

1. **`SessionStart`** (`session-start`): Lưu baseline git diff khi bắt đầu phiên để các thay đổi có sẵn trước đó không bị review nhầm ở các lượt chỉ hỏi-đáp.
2. **`PostToolUse` / `afterFileEdit`** (`track`): Theo dõi các file được agent chỉnh sửa (hỗ trợ cả `toolCall.args`, `tool_input`, `apply_patch`, và `file_path` của Cursor), bỏ qua artifact, file ngoài workspace và `ignore_patterns`.
3. **`Stop` / `stop`** (`review`): Khi agent kết thúc lượt và có thay đổi code mới:
   - Gom `git status`, `git diff`, và nội dung file mới tạo.
   - Gọi AI reviewer CLI tương ứng với `AI_REVIEW_HOOK_ACTIVE=1` để chống đệ quy.
   - Lưu báo cáo đầy đủ vào `<artifactDirectoryPath>/ai-code-review.md` (hoặc `/tmp/agy-ai-review/<hash>-report.md` nếu runtime không có `artifactDirectoryPath`).
   - Nếu reviewer trả về `VERDICT: NEEDS_FIX`: chặn dừng và yêu cầu agent tự sửa toàn bộ finding (kể cả `LOW`/`NIT`, tối đa `max_review_rounds = 5` vòng/lượt).
   - Nếu `VERDICT: PASS` hoặc gặp lỗi CLI/timeout: fail-open cho phép dừng.

## Cấu hình & Đổi Model / Provider

Chỉnh sửa file `.agents/ai-review-config.json`:

- **Đổi reviewer theo từng runtime (`reviewer_by_runtime`)** (mặc định chỉ cần khai báo ngoại lệ `claude -> codex`, còn lại kế thừa `provider`/`model` cấp trên):
  ```json
  {
    "provider": "claude",
    "model": "",
    "reviewer_by_runtime": {
      "claude": { "provider": "codex", "model": "" }
    }
  }
  ```
- **Dùng CLI tùy chỉnh (`custom`):**
  ```json
  {
    "provider": "custom",
    "model": "my-model",
    "custom_command": ["my-cli", "--model", "{model}", "-p"]
  }
  ```

### Biến môi trường ghi đè nhanh

- `AI_REVIEW_DISABLED=1`: Tạm tắt hook trên mọi runtime.
- `AI_REVIEW_PROVIDER=claude|codex|custom`: Ghi đè provider (ưu tiên cao hơn `reviewer_by_runtime`).
- `AI_REVIEW_MODEL=<model>`: Ghi đè model.
- `AI_REVIEW_MAX_ROUNDS=<n>`: Đổi số vòng review tối đa (mặc định `5`).
- `AI_REVIEW_INCLUDE_NITS=0|1`: Bật/tắt chế độ bắt buộc sửa cả lỗi nhỏ (`LOW`/`NIT`), mặc định `1` (`true`).
- `AI_REVIEW_TIMEOUT=<seconds>`: Đổi timeout gọi CLI.

## Kiểm thử

```bash
python3 -B -m unittest discover -s .agents/scripts -p 'test_*.py' -v
```
