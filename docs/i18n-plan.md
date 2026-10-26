# Plan: translations and Simplified Chinese

Goal: let people use Wu, its website and its README in Simplified Chinese, with a setting to pick the app's language, and set things up so more languages can be added later without redoing the work.

Every phase can ship on its own. Anything not yet translated shows in English, so a half-done state is never broken.

## What gets translated and what doesn't

Translate:

- Labels, buttons, tooltips, menus, context menus, placeholders, empty states and headers
- Notifications, toasts and confirm dialogs (`window.prompt`)
- Settings page names, section headers, setting titles and descriptions, and dropdown choices
- Command palette and keymap editor command names, and command descriptions in the keymap editor
- Relative times ("Just now", "2 hours ago") and status bar text
- The app menu bar, dock menu and onboarding

Keep in English:

- Setting keys in `settings.json`, command names in keymap files, and anything else a user types into a config file
- Log messages, and error text that is only written to the log
- Text that comes from other programs: git, language servers, compilers, and agent CLI output
- Product and brand names: Wu, Claude Code, Codex, OpenCode, Git, GitHub, theme and font names
- Comments in `assets/settings/default.json`, command descriptions in `keymap.json` hover help, test strings and component previews

## Phase 0: ground rules

1. **The word list.** Fixed Chinese words for core terms, so the app, website and README all use the same word. Review it with the translator before the first pass:

   | English | Chinese |
   | --- | --- |
   | Project | 项目 |
   | Workspace | 工作区 |
   | Panel | 面板 |
   | Pane | 窗格 |
   | Dock | 停靠栏 |
   | Tab | 标签页 |
   | Buffer | 缓冲区 |
   | Stage / Unstage | 暂存 / 取消暂存 |
   | Commit | 提交 |
   | Branch | 分支 |
   | Worktree | 工作树 |
   | Diff | 差异 |
   | Terminal | 终端 |
   | Agent | 智能体 |
   | Chat | 对话 |
   | Pin / Archive | 置顶 / 归档 |

2. **Writing rules for Chinese text.** Use full-width punctuation (，。：？！) in sentences. Put a space between Chinese and Latin letters or numbers ("已更改 3 个文件"). Keep keyboard shortcuts and file names as they are.
3. **Traditional Chinese.** If the system language is any form of Chinese, including Traditional Chinese (zh-TW, zh-HK), Wu shows Simplified Chinese.
4. **Who reviews.** A native speaker checks the first full pass before release.

## Phase 1: the foundation

### 1.1 A new `i18n` crate

Add `crates/i18n/src/i18n.rs` (set with `[lib] path` in its `Cargo.toml`). It should depend on almost nothing, so that any crate can use it, including `ui`, `time_format` and `settings_ui`. In particular it should not depend on `gpui`.

- **English text is the key.** You write `t!("Save")`. If the current language is English, or there's no translation for that text, it returns the English. So the code stays easy to read, nobody has to invent key names, and a missing translation just shows English.
- **Placeholders:** `t!("{count} files changed", count = changed.len())`. Each translation must keep the same `{names}`, but it can put them in a different order.
- **The same English word with two meanings:** `tc!("git", "Stage", ...)` adds a short context, so "Stage" in git and "Stage" somewhere else can get different Chinese words. Use it only when it's needed.
- **Plurals:** keep the English code as it is, with one string for "1 minute ago" and another for "{count} minutes ago". Both map to the same Chinese text. Chinese has no plural forms, so no plural system is needed.
- **Text stored in tables:** for `&'static str` text kept in tables (settings page data, menu definitions), mark it with `n!("...")`. That does nothing at runtime, but lets the check script find it. Translate it where it's shown with `i18n::translate(text)`.
- **Language files:** each language is one file, `assets/locales/zh-CN.json`, built into the app. It's a flat list of English text to Chinese text. Command names and descriptions go in a second file, `assets/locales/zh-CN.actions.json`, keyed by command name (`editor::ToggleComments`), with a `name` and a `description` for each.
- **Switching:** the current language is kept in one global value, swapped with `i18n::set_language(...)`.
- **Speed and memory:** a build step turns the JSON into sorted, read-only tables inside the app, so nothing is parsed or copied at startup. Each `t!` call remembers its last translation for the current language, so later renders skip the table search.
- **Why not `rust-i18n` or Fluent:** both work. They are built for many languages with complex plural rules, and Wu doesn't need that. A small crate of our own fits text stored in `&'static str` tables and lookups at runtime better, and adds no dependencies. Worth reconsidering if many languages are added later.

### 1.2 The setting

- Add `ui_language` to `settings_content`, with the choices `"system"` (the default), `"en"` and `"zh-CN"`. Add it to `assets/settings/default.json`. Don't add it to `initial_user_settings.json`, because the default should keep following the system language.
- With `"system"`, use `sys-locale` (already a dependency). Any language starting with `zh` gets Simplified Chinese, everything else gets English (see Phase 0, point 3).
- Show it at the top of Settings > Appearance as a dropdown. Each language is listed in its own language: "English", "简体中文".
- Tests always start in English and never read the system language, so existing tests that check for English text keep passing.

### 1.3 Switching without a restart

When the setting changes:

1. Call `i18n::set_language`.
2. Rebuild the menu bar and dock menu. This is the same `app_menus(cx)` and `cx.set_menus(...)` call that `reload_keymaps` in `crates/wu/src/wu.rs` already makes.
3. Rebuild the settings search index.
4. Redraw all windows.

Text that was saved when something was opened updates the next time it's opened. That includes notifications already on screen, placeholders in pickers that are already open, and placeholder text in editors that are already open. The setting description says: "Some parts update after you reopen them or restart Wu."

### 1.4 Fonts

- Geist and JetBrains Mono have no Chinese characters, so the system picks a fallback font. On macOS it can pick a Japanese font, which draws some characters differently from the way Chinese readers expect. To avoid that, when Chinese is on and the user hasn't set `ui_font_fallbacks` or `buffer_font_fallbacks`, use:
  - **macOS:** PingFang SC
  - **Windows:** Microsoft YaHei UI
  - **Linux:** Noto Sans CJK SC

  Do this where the theme settings are read, not by writing to the user's settings file.
- Line wrapping already breaks between Chinese characters. Add a rule in `crates/gpui/src/text_system/line_wrapper.rs` so a line never starts with closing punctuation like ，。、；：？！）」』》, and never ends with opening punctuation like （「『《.

### 1.5 Checks and tools

- **`script/check-translations`** scans the code for `t!`, `tc!` and `n!` text, and lists:
  - text with no Chinese translation;
  - translations that are no longer used;
  - translations whose `{placeholders}` don't match the English.

  Run it before every release by adding it to `.github/workflows/release.yml`. It can also run locally.
- **Test language (debug builds only):** add a test language that shows every translated string inside brackets, like `⟦Save⟧`. Any plain English left on screen is a string that hasn't been wrapped yet. This is the quickest way to find gaps.
- **Unit tests in `i18n`:**
  - falling back to English;
  - filling placeholders;
  - context keys;
  - switching languages;
  - the system language choice for `zh`, `zh-CN`, `zh-Hans`, `zh-TW`, `zh-HK` and non-Chinese languages.

## Phase 2: shared UI

The parts everyone sees in every window. Roughly 800–1,000 strings.

- **`ui`:** shared components like empty states, confirm buttons and the "Close" / "Cancel" defaults. Skip component previews.
- **`workspace`:** tabs, pane menus, notifications, the save, close and discard dialogs, and dock menus.
- **`title_bar`, `wu`:** the menu bar, dock menu, and "About" and quit dialogs.
- **`time_format`:** relative times. Absolute dates should follow the UI language instead of only the system language.
- **`picker`, `file_finder`, `command_palette`, `recent_projects`, `onboarding`, `notifications`.**

Rules while wrapping:

- Never build a sentence from pieces, like `"Delete " + name + "?"`. Chinese uses a different word order. Use one string with a placeholder: `t!("Delete {name}?", name = name)`.
- Look out for fixed widths (`w(px(...))`) on buttons, tabs and badges. Chinese text is often shorter, but some labels get longer.
- `.to_uppercase()` on headers is harmless for Chinese text.

## Phase 3: agent chat

About 250 strings in `agent_chat`, plus the parts of `agent_harness` that users see (status lines, permission prompts, error banners, the model picker, and the sidebar with its sections, pins and archive).

- **Saved text:** check for English text that's saved to disk and shown later. For example, if the default section names are saved, existing users would keep seeing them in English. Translate when the text is shown, not when it's saved.
- **Chat titles:** titles that the agent or the user wrote stay as they are.
- **Agent output:** what the agent writes is not translated.

## Phase 4: settings, keymap editor and commands

- **`settings_ui`:** about 820 titles and descriptions in `page_data.rs`, plus page names and section headers.
  - Mark them all with `n!` and translate them where they're shown (`SettingItem.title` and `description` are `&'static str`).
  - Dropdown choices come from `strum::VariantNames`. Translate them by `TypeName::Variant` key, so that the same word in two different settings can differ.
  - Settings search should match both the Chinese and the English text. Many people know the English setting names from docs and forum posts.
- **Command palette:** names come from `humanize_action_name` in `crates/command_palette/src/command_palette.rs`. Show the Chinese name from `zh-CN.actions.json`, with the English name next to it in muted text. Search matches both: run the fuzzy match against both names and keep the best score.
- **Keymap editor:** use the same Chinese command names, and search both names in the same way.
- **Command descriptions:** the keymap editor shows each command's doc comment as a tooltip (`cx.action_documentation()` in `crates/keymap_editor/src/keymap_editor.rs`). Show the Chinese description from `zh-CN.actions.json` there instead. The hover help inside `keymap.json` stays in English, since that's a config file.
- **About 1,000+ commands,** each with a name and a description. `script/check-translations` also lists commands with no Chinese name or description.

## Phase 5: the rest of the panels

Done in order of how often people see them:

1. `project_panel`, `search`, `editor` (context menu, hover popups, inline messages, find bar)
2. `git_ui`, `git_ui_core` (panel, commit box, branch and remote pickers, sync menu)
3. `terminal_view`, `outline_panel`, `diagnostics`, `file_finder` leftovers
4. `extensions_ui`, `language_tools`, `keymap_editor` leftovers, `task`, `recent_projects`, `remote`
5. `debugger_ui`, `dap_adapters` (only the parts users see)

The test language from 1.5 shows what's left after each crate.

## Phase 6: system pieces and final polish

- **macOS:**
  - Add `CFBundleLocalizations` (`en`, `zh-Hans`) to a plist in `crates/wu/resources/info/`. This lets the items that macOS adds itself show in Chinese: Services, Hide, Emoji & Symbols, Start Dictation, and open and save dialogs.
  - These follow the macOS language by default. To make them follow Wu's setting instead, set the app's `AppleLanguages` value at startup. That only applies after a restart.
- **Windows:** the "New Window" jump list item. System dialogs follow the Windows language.
- **Linux:** add `Name[zh_CN]` and `Comment[zh_CN]` to `crates/wu/resources/zed.desktop.in`.
- **Layout pass:** go through each main screen in Chinese and fix any clipped text, uneven row heights or crowded buttons.
- **Translation review:** a native speaker reads through the whole UI against the word list.
- **Changelog:** add "Wu is now available in Simplified Chinese." under Unreleased.

## Phase 7: website and README

### Website (`website/`)

- **Chinese pages:** `/zh/`, `/zh/docs` and `/zh/support`, built next to the English pages from the same components. Add them to `website/src/routes.tsx`, with Chinese page titles and descriptions.
- **Text in one place:** move the page text out of the components into `website/src/i18n/en.ts` and `website/src/i18n/zh-CN.ts`. Pages pick the text for their language. A type check makes sure the Chinese file has every key the English one has.
- **Pseudo UI:** the Wu and agent windows on the home page (`WuWindow.tsx`, `AgentWindow.tsx`, `wuWindowData.ts`) show Chinese menus, panels and chat text on the Chinese pages. Use the same words as the app, so the screenshots match what people get.
- **Language switch:** a small "English / 简体中文" link in the header and footer that goes to the same page in the other language.
- **Page details:**
  - set `<html lang="zh-CN">` on the Chinese pages;
  - add `hreflang` links between each English page and its Chinese copy, so search engines show the right one;
  - make sure the site fonts have a Chinese fallback (PingFang SC, Microsoft YaHei, Noto Sans CJK SC).
- **Copy:** same rules as the English site. Describe what people get, not specs, and keep it plain and simple.
- **Check:** `bun run build` passes, and the type check catches missing keys.

### README

- Add `README.zh-CN.md`, a full translation of `README.md`.
- Add a "English | 简体中文" line at the top of both files, linking to each other.
- When `README.md` changes, update `README.zh-CN.md` in the same change.

## How to split the work

- Each phase, and each crate within a phase, is its own change, small enough to review on its own.
- The wrapping and first-pass translation can go to agents, one crate each. Give each one the word list, the writing rules, the "what not to translate" list and this rule, word for word:

  > Comments should be rare. Default to zero. A comment is allowed only when the code cannot be made self-explanatory and a reader would otherwise get it wrong: a hidden constraint, a non-obvious ordering, a workaround for an external quirk. Keep it to one short line. Never explain what the code does, never describe a design or a range of values, never narrate a fix. If a good name or a small restructure removes the need, do that instead.

- After each crate: run that crate's tests and `./script/clippy -p <crate>`, then `script/check-translations`.

## Risks

- **Missed strings:** the test language and the check script catch most of them. Anything missed just shows English.
- **Tests that expect English:** safe, because tests always run in English.
- **Saved English text:** see the saved-text point in Phase 3. Translate text when it's shown, not when it's saved.
- **Layout:** some places have fixed sizes. The layout pass in Phase 6 covers this.
- **Keeping it up:** every new feature needs its Chinese text. The check script fails the release if any is missing.
