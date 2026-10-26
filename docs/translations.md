# Translations

Wu's interface can be shown in English or Simplified Chinese (`"ui_language"` in settings). English is written in the code. Chinese text lives in `assets/locales/zh-CN/`.

## Writing translatable code

The `i18n` crate does the lookups. Add `i18n.workspace = true` to the crate's `Cargo.toml` and use its macros:

```rust
use i18n::t;

Label::new(t!("Save"))
Button::new("discard", t!("Discard Changes"))
t!("{count} files changed", count = changed.len())
t!("Delete {name}?", name = file_name)
```

- `t!("English text")` returns the text in the current language as a `SharedString`, or the English text if there's no translation.
- Placeholders are `{name}` with `name = value` arguments. The value can be anything that implements `Display`. Placeholder names are lower case letters, digits and `_`.
- `tc!("context", "English text")` is for the same English text that needs different Chinese in different places, like `tc!("tab", "Pin")` (固定) and `tc!("chat", "Pin")` (置顶). Use it only when it's needed.
- `n!("English text")` returns the English `&'static str` unchanged. Use it for text that's stored in a table or a `&'static str` field and shown later. Translate it where it's shown with `i18n::translate(text)`.
- `i18n::lookup(text)` returns `Some(translation)` or `None`, for text that isn't `'static`.
- Command names and descriptions come from `i18n::action_name`, `i18n::action_namespace` and `i18n::action_description`.

Rules:

- The text inside the macro must be a plain string literal, not a raw string, `concat!` or a constant. That's how the checks find it.
- Never build a sentence from pieces like `format!("Delete {}?", name)` or `"Open " + label`. Chinese puts words in a different order. Use one string with placeholders.
- Keep English plurals as separate strings, like `t!("1 file")` and `t!("{count} files", count = n)`. Both can map to the same Chinese text.
- Don't translate: setting keys, command names in keymap files, log messages, error text that only goes to the log, text from other programs (git, language servers, compilers, agent CLIs), product names (Wu, Claude Code, Codex, OpenCode, Git, GitHub, macOS), theme and font names, file names, test strings and component previews.
- Tests always run in English, so tests that check English text keep working.

## Adding the Chinese text

Each crate has its own file, `assets/locales/zh-CN/strings/<crate>.json`. It's a flat JSON object from the English text to the Chinese text, with keys sorted, two-space indentation, and Chinese characters written as they are, not escaped:

```json
{
  "Discard Changes": "放弃更改",
  "tab|Pin": "固定",
  "{count} files changed": "已更改 {count} 个文件"
}
```

Text from `tc!` uses the key `context|English text`.

The JSON files are turned into fixed tables when the `i18n` crate builds, so after editing one, rebuild Wu to see the change.

Commands go in `assets/locales/zh-CN/actions/<namespace>.json`, keyed by the full command name:

```json
{
  "editor::ToggleComments": {
    "name": "切换注释",
    "description": "为所选行添加或删除注释。"
  }
}
```

Namespace names, like `"editor": "编辑器"`, go in `assets/locales/zh-CN/action_namespaces.json`.

The hover help in settings.json comes from the settings' doc comments. Their Chinese lives in `assets/locales/zh-CN/schema/`, a flat JSON object from the full English description to the Chinese. Keep setting names, values and code unchanged, and write "Default:" as "默认值：". The `settings` crate's tests fail when a description has no Chinese or a translation no longer matches any description, so after changing a doc comment in `settings_content`, run `cargo test -p settings` and update the file.

## Checks

- `cargo test -p i18n` fails when:
  - a `t!`, `tc!` or `n!` text has no Chinese translation;
  - a translation is no longer used;
  - placeholders don't match the English;
  - the same English text has two different translations.
- `script/check-translations` also checks that every command has a Chinese name and description. It builds Wu to list the commands. Pass `--binary target/debug/wu` to use an existing build, or `--strings-only` to skip the command check.
- Run a debug build with `WU_PSEUDO_TRANSLATION=1` to show every translated string in brackets, like `⟦Save⟧`. Any plain English left on screen hasn't been made translatable yet.

## Chinese style

- Use full-width punctuation in sentences: ，。：；？！（）“”.
- Put a space between Chinese and Latin letters or numbers: "已更改 3 个文件", "打开 Git 面板".
- Keep keyboard shortcuts, file names, paths and code as they are.
- Keep it short, plain and natural. Prefer the words people know from VS Code's Chinese interface.
- Title Case in English has no Chinese equivalent. Just translate the words.
- Don't end button labels, menu items or titles with "。". Do end full sentences in descriptions with it.

## Word list

Use these words everywhere, so the whole app says the same thing.

| English | Chinese |
| --- | --- |
| Agent | 智能体 |
| Agent chat | 智能体对话 |
| Chat | 对话 |
| Message | 消息 |
| Model | 模型 |
| Thinking / Thought | 思考 |
| Context window | 上下文窗口 |
| Permission | 权限 |
| Allow / Deny | 允许 / 拒绝 |
| Fork (a chat) | 分叉 |
| Side chat | 侧边对话 |
| Skill | 技能 |
| Slash command | 斜杠命令 |
| Mention | 提及 |
| Attach / Attachment | 附加 / 附件 |
| Image | 图片 |
| Pin / Unpin (chats, messages) | 置顶 / 取消置顶 |
| Pin / Unpin (tabs) | 固定 / 取消固定 |
| Archive | 归档 |
| Section (in the chat sidebar) | 分组 |
| Save (a message) | 收藏 |
| Saved messages | 已收藏的消息 |
| Project | 项目 |
| Workspace | 工作区 |
| Worktree | 工作树 |
| Window | 窗口 |
| Panel | 面板 |
| Pane | 窗格 |
| Dock | 停靠栏 |
| Sidebar | 侧边栏 |
| Activity bar | 活动栏 |
| Status bar | 状态栏 |
| Title bar | 标题栏 |
| Tab / Tab bar | 标签页 / 标签栏 |
| Toolbar | 工具栏 |
| Project panel | 项目面板 |
| File tree | 文件树 |
| Outline | 大纲 |
| Editor | 编辑器 |
| Buffer | 缓冲区 |
| Multibuffer | 多缓冲区 |
| Split | 拆分 |
| Zoom | 缩放 |
| Cursor | 光标 |
| Selection | 选区 |
| Line / Column | 行 / 列 |
| Gutter | 装订线 |
| Minimap | 缩略图 |
| Scrollbar | 滚动条 |
| Soft wrap | 自动换行 |
| Indent | 缩进 |
| Format | 格式化 |
| Completion | 补全 |
| Snippet | 代码片段 |
| Hover | 悬停 |
| Inlay hint | 内嵌提示 |
| Go to definition | 转到定义 |
| References | 引用 |
| Symbol | 符号 |
| Diagnostics | 诊断 |
| Error / Warning | 错误 / 警告 |
| Language server | 语言服务器 |
| File / Folder | 文件 / 文件夹 |
| Open / Close | 打开 / 关闭 |
| Save / Save As | 保存 / 另存为 |
| Rename | 重命名 |
| Delete | 删除 |
| Move to Trash | 移到废纸篓 |
| Copy / Cut / Paste | 复制 / 剪切 / 粘贴 |
| Undo / Redo | 撤销 / 重做 |
| Search / Find / Replace | 搜索 / 查找 / 替换 |
| Settings | 设置 |
| Keybinding | 快捷键 |
| Keymap | 键位映射 |
| Command | 命令 |
| Command palette | 命令面板 |
| Extension | 扩展 |
| Theme / Icon theme | 主题 / 图标主题 |
| Font | 字体 |
| Terminal | 终端 |
| Task | 任务 |
| Debugger / Breakpoint | 调试器 / 断点 |
| Repository | 仓库 |
| Remote | 远程仓库 (git), 远程 (remote machines) |
| Branch | 分支 |
| Commit | 提交 |
| Stage / Unstage | 暂存 / 取消暂存 |
| Discard changes | 放弃更改 |
| Push / Pull / Fetch | 推送 / 拉取 / 获取 |
| Sync | 同步 |
| Merge / Rebase | 合并 / 变基 |
| Stash | 储藏 |
| Checkout / Switch branch | 签出 / 切换分支 |
| Conflict | 冲突 |
| Diff | 差异 |
| History | 历史记录 |
| Blame | 逐行追溯 |
| Hunk (diff) | 差异块 |
| Excerpt (multibuffer) | 摘录 |
| Breadcrumbs | 痕迹导航 |
| Code action | 代码操作 |
| Signature help | 签名帮助 |
| Indent guides | 缩进参考线 |
| Inline | 行内 |
| Amend / Uncommit | 修正提交 / 撤销提交 |
| Sign off | 签署 |
| Pull request | 拉取请求 |
| Permalink | 永久链接 |
| Call stack | 调用堆栈 |
| Watch (debugger) | 监视 |
| Step into / over / out / back | 单步执行 / 单步跳过 / 单步跳出 / 单步后退 |
| Focus (move focus to) | 聚焦到 |
| Notification | 通知 |
| Update / Restart / Quit | 更新 / 重启 / 退出 |
| Help / Documentation | 帮助 / 文档 |
| Recent projects | 最近的项目 |
| Toggle X (command) | 切换X |
| Show / Hide | 显示 / 隐藏 |
| Expand / Collapse | 展开 / 折叠 |
| Enable / Disable | 启用 / 停用 |
| Default | 默认 |
| System | 跟随系统 |
| None | 无 |
| Loading… | 正在加载… |
| OK / Cancel | 确定 / 取消 |
| Dismiss | 关闭 |
| Learn More | 了解详情 |
| Reveal in Finder | 在访达中显示 |
| No items | 暂无内容 |
| Note / Tip / Important / Warning / Caution | 备注 / 提示 / 重要 / 警告 / 注意 |
| Just now | 刚刚 |
| Don't show again (suppress) | 不再显示 |
| Restricted Mode / Trust | 受限模式 / 信任 |
| Onboarding | 新手引导 |
| Dictation / Dictate | 听写 |
| Code Lens | CodeLens |
| Format (a file) | 格式化文档 |
| Unset breakpoint | 移除断点 |
| Effort / Reasoning (models) | 推理强度 |
| Restore (session, workspace, terminals) | 恢复 |
| Restore (git changes, trash, files) | 还原 |
| Zoom in / out (pane maximize) | 最大化窗格 / 还原窗格 |
| Zoom in / out (font, image) | 放大 / 缩小 |
| Fold / Unfold directory (compact folders) | 合并文件夹 / 取消合并文件夹 |
| Prompt (a message sent to an agent) | 消息 |
| Chat box (agent composer) | 对话输入框 |
| Application menu | 应用菜单 |
| Settings profile | 设置配置文件 |
| Rewrap | 重新换行 |
| Approve for me / Bypass permissions | 代我批准 / 绕过权限 |
| On / Off (short labels) | 开 / 关 |
| Press Enter | 按 Enter |
| {modifier}-click | 按住 {modifier} 点击 |
| Journal | 日记 |
| Untitled | 未命名 |
| Untitled chat | 未命名对话 |
| Bookmark | 书签 |
| Unified / Split (diff view) | 统一 / 拆分 |
| Restore | 还原 |
| Run to Cursor | 运行到光标处 |
