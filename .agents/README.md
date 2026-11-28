# AI Code Review Hook (`.agents/hooks.json`)

Hook tự động gọi một AI model khác (mặc định: **Claude Code CLI**) để review code mỗi khi agent vừa hoàn thành việc sửa/viết code trong workspace `rust-mantis-bt`.

## Cách hoạt động

1. **`SessionStart`** (`session-start`): Lưu baseline git diff khi bắt đầu hội thoại để các thay đổi có sẵn trước đó không bị review nhầm ở các lượt chỉ hỏi-đáp.
2. **`PostToolUse`** (`track`): Theo dõi các file được agent chỉnh sửa qua `write_file`, `edit_file`, `write_to_file`, `replace_file_content`, `multi_replace_file_content`, `apply_patch` (bỏ qua artifact, file ngoài workspace và các file khớp `ignore_patterns`).
3. **`Stop`** (`review`): Khi agent kết thúc lượt (`terminationReason == "NO_TOOL_CALL"` và `fullyIdle != false`), nếu có thay đổi code mới:
   - Gom `git status`, `git diff`, và nội dung file mới tạo.
   - Gọi AI reviewer CLI (`claude`, `codex`, hoặc `custom`) với `AI_REVIEW_HOOK_ACTIVE=1` để chống đệ quy.
   - Lưu báo cáo đầy đủ vào `<artifactDirectoryPath>/ai-code-review.md`.
   - Nếu reviewer trả về `VERDICT: NEEDS_FIX`: trả về `{"decision": "continue", "reason": "..."}` để chặn dừng và yêu cầu agent tự sửa toàn bộ finding (kể cả `LOW`/`NIT`, tối đa `max_review_rounds = 5` vòng/lượt).
   - Nếu `VERDICT: PASS` hoặc gặp lỗi CLI/timeout: fail-open với `{"decision": "allow"}`.

## Cấu hình & Đổi Model / Provider

Chỉnh sửa file `.agents/ai-review-config.json`:

- **Mặc định (Claude CLI default model):**
  ```json
  {
    "provider": "claude",
    "model": ""
  }
  ```
- **Chỉ định model Claude cụ thể** (ví dụ `sonnet` hoặc `opus`):
  ```json
  {
    "provider": "claude",
    "model": "sonnet"
  }
  ```
- **Đổi sang OpenAI Codex CLI:**
  ```json
  {
    "provider": "codex",
    "model": ""
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

- `AI_REVIEW_DISABLED=1`: Tạm tắt hook.
- `AI_REVIEW_PROVIDER=claude|codex|custom`: Đổi provider tạm thời.
- `AI_REVIEW_MODEL=<model>`: Đổi model tạm thời.
- `AI_REVIEW_MAX_ROUNDS=<n>`: Đổi số vòng review tối đa (mặc định `5`).
- `AI_REVIEW_INCLUDE_NITS=0|1`: Bật/tắt chế độ bắt buộc sửa cả lỗi nhỏ (`LOW`/`NIT`), mặc định `1` (`true`).
- `AI_REVIEW_TIMEOUT=<seconds>`: Đổi timeout gọi CLI.

Để tắt vĩnh viễn, đặt `"enabled": false` trong `.agents/hooks.json` hoặc `.agents/ai-review-config.json`.

## Kiểm thử

```bash
python3 -m unittest discover -s .agents/scripts -p 'test_*.py' -v
```
