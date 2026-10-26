import type { ChatStep, MockChat } from '../components/wuWindowData';
import { INSTALL_COMMAND, INSTALL_GUIDE_URL, ISSUES_URL, RAYCAST_URL, ZED_DOCS_URL } from '../consts';
import type { Messages } from './en';

export const zhCN: Messages = {
  meta: {
    home: {
      title: 'Wu：快速的原生代码编辑器和智能体工作区',
      description: 'Wu 是一款快速的原生代码编辑器，用 Rust 编写。开箱即用，内置编程智能体。无需账号，没有遥测。'
    },
    docs: { title: 'Wu 文档', description: 'Wu 和 Zed 有哪些不同：去掉了什么，改了什么，文件放在哪里。' },
    support: { title: '支持 Wu', description: 'Wu 免费开源。在 Open Collective 上捐助，帮它继续走下去。' },
    notFound: { title: '找不到页面 · Wu' }
  },
  languageName: '简体中文',
  header: {
    docs: '文档',
    support: '支持',
    download: '下载'
  },
  footer: {
    navLabel: '页脚',
    product: { title: 'Wu', download: '下载', docs: '文档', releases: '版本发布' },
    resources: {
      title: '资源',
      installGuide: '安装指南',
      memoryBenchmark: '内存测试',
      raycast: 'Raycast 扩展'
    },
    source: { title: '源代码', github: 'GitHub', reportIssue: '报告问题', support: '支持 Wu' },
    license: 'Wu 采用 GPL-3.0-or-later 许可证。'
  },
  installCommand: { copied: '已复制', copy: '复制', copyLabel: '复制安装命令' },
  downloads: {
    macArm: 'macOS（Apple 芯片）',
    macIntel: 'macOS（Intel 芯片）',
    windows: 'Windows（x86-64）',
    linuxX86: 'Linux（x86-64）',
    linuxArm: 'Linux（AArch64）'
  },
  home: {
    title: '快速的原生代码编辑器和智能体工作区',
    tagline: '用 Rust 编写，开箱即用：内置编程智能体，用起来顺手，第一天就有你需要的一切。',
    installManually: (href: string) => `或者[手动安装](${href})`,
    downloadFor: (platform: string) => `下载 ${platform} 版`,
    downloadWu: '下载 Wu',
    allPlatforms: '所有平台',
    memory: {
      title: '占用内存少',
      body: '在我们做过的每一项测试里，Wu 用的内存都比 Zed 和 VS Code 少。',
      link: '查看测试结果'
    },
    agents: {
      title: '内置你的编程智能体',
      body: '在代码旁边直接使用 Claude Code、Codex 和 OpenCode，用的就是你已经登录的账号。',
      link: '了解用法'
    },
    openSource: {
      title: '开源，基于 Zed',
      body: 'Wu 是 Zed 的分支。它保留了 Zed 的编辑器核心、GPU 渲染和语言工具，所有 Zed 扩展都能在 Wu 里使用。',
      source: '查看源代码',
      docs: '阅读文档'
    },
    ready: {
      iconAlt: 'Wu 应用图标',
      title: '随时可以开始',
      bodyScript: 'Wu 免费，一分钟就能装好。在终端里运行这条命令。',
      bodyPick: 'Wu 免费，一分钟就能装好。选择你的平台。',
      orInstallManually: '或者手动安装',
      alsoRun: '在 macOS 或 Linux 上，也可以运行',
      firstTime: `第一次在 macOS 或 Linux 上安装？请看[安装指南](${INSTALL_GUIDE_URL})。`
    }
  },
  memoryChart: {
    label: '内存占用（MiB），越低越好',
    caption: '进程占用的总内存，越低越好。',
    scenarios: { idle: '空闲时', search: '搜索仓库时' }
  },
  faq: {
    title: '关于 Wu 的常见问题',
    questions: [
      {
        question: 'Wu 免费吗？',
        answer: '免费。Wu 是开源软件，采用 GPL-3.0-or-later 许可证，和 Zed 一样。没有付费版本。'
      },
      {
        question: 'Wu 会收集数据吗？',
        answer: '不会。Wu 没有遥测，没有崩溃报告，也不需要账号。它从不把你的使用数据发送到任何地方。'
      },
      {
        question: 'Wu 支持 AI 编程智能体吗？',
        answer:
          '支持。Wu 内置了对 Claude Code、Codex 和 OpenCode 等编程智能体的支持。不过，Wu 不支持 Zed 的 AI 功能，比如行内助手和编辑预测。'
      },
      {
        question: 'Zed 扩展能在 Wu 里用吗？',
        answer: '能。Wu 从 Zed 扩展商店安装扩展，所有 Zed 扩展都能用。'
      },
      {
        question: '可以同时保留 Zed 吗？',
        answer:
          '可以。Wu 和 Zed 的设置是分开的，分别在 `~/.config/wu` 和 `~/.config/zed`。如果想让两边设置一样，把你的 `settings.json` 和 `keymap.json` 复制过去就行。'
      },
      {
        question: '为什么 macOS 提示无法打开 Wu？',
        answer: `Wu 还没有用 Apple 开发者证书签名，所以 macOS 第一次会拦住它。[安装指南](${INSTALL_GUIDE_URL})里有一次性的解决办法。`
      },
      {
        question: 'Wu 和 Zed 有什么不同？',
        answer:
          'Wu 去掉了 Zed 的 AI 服务、协作和遥测功能。它有自己的智能体对话，基于 Claude Code、Codex 和 OpenCode 的命令行工具，也有自己的外观，还在其他好几个地方比 Zed 更好用。详情请看[文档](/zh/docs/)。'
      },
      {
        question: '在哪里报告问题？',
        answer: `在 [GitHub](${ISSUES_URL}) 上提交 issue。提交前请先看看有没有人已经报告过。`
      }
    ]
  },
  docs: {
    title: '文档',
    intro: `这里介绍的是 Wu 特有的内容。Wu 和 Zed 共用编辑器核心，所以设置、快捷键、语言、扩展、任务、调试和远程开发这些内容，[Zed 文档](${ZED_DOCS_URL})同样适用于 Wu。`,
    gettingStarted: {
      title: '开始使用',
      body: `在 macOS 和 Linux 上，在终端里运行 \`${INSTALL_COMMAND}\`。也可以下载对应平台的 Wu，然后按照[安装指南](${INSTALL_GUIDE_URL})操作。`,
      paths: {
        commandLine: '命令行',
        settingsUnix: '设置（macOS、Linux）',
        settingsWindows: '设置（Windows）',
        appDataMac: '应用数据（macOS）',
        projectSettings: '项目设置'
      }
    },
    agentChat: {
      title: '智能体对话',
      intro:
        '在 Wu 里和 Claude Code、Codex 或 OpenCode 对话。它用的是你已经安装并登录的命令行工具，所以不需要 API 密钥，也不需要 Wu 账号。',
      items: [
        '**开始对话：**点击活动栏里的智能体图标。用“继续已保存的对话”可以接着在终端里开始的对话继续。',
        '**输入** `/` 使用命令，`@` 提及文件，`$` 使用技能。粘贴或拖入图片即可附加。',
        '**智能体工作时也能发送：**你的消息会排队等待。你可以编辑它，或者用它来引导当前这一轮。',
        '**听写：**按 `Cmd-D`（Windows 和 Linux 上是 `Ctrl-D`）。它在你的电脑上本地运行，需要 Apple 芯片的 macOS 14 或更高版本、Linux 或 Windows。',
        '**模型选择器**用来设置模型、推理强度和权限。旁边的圆环显示上下文和套餐用量。',
        '**对话**可以置顶、分叉和重命名。子智能体会在自己的标签页里打开。'
      ],
      outro: '通知、自动压缩、Claude in Chrome、隐藏智能体以及关闭智能体对话，都在设置的“智能体对话”页面里。'
    },
    workspace: {
      title: '工作区',
      items: [
        '侧边的活动栏用来切换项目、Git、大纲、搜索、调试和智能体面板。面板默认停靠在左侧，`activity_bar.icon_size` 用来设置图标大小。',
        '编辑器和终端标签栏里的 `+` 按钮位于最后一个标签页之后，点击直接新建文件或终端，不弹出菜单。',
        '`Cmd-W` / `Ctrl-W` 只关闭编辑器标签页，不会关闭侧边面板。'
      ]
    },
    extensions: {
      title: '扩展',
      body: 'Wu 从 Zed 扩展商店安装扩展，不用做任何修改就能在 Wu 里使用。'
    },
    privacy: {
      title: '隐私',
      body: 'Wu 没有账号，没有遥测，也没有崩溃报告。它从不把你的使用数据发送到任何地方。智能体对话只和你所用工具背后的服务商通信。'
    },
    notIncluded: {
      title: '不包含的功能',
      items: ['协作：频道、通话、屏幕共享和共享项目。', 'Vim 和 Helix 模式。', '开发容器。', 'REPL 和日记。'],
      note: '如果 Zed 文档里的某个设置或快捷键属于这些功能，它在 Wu 里不起作用。'
    },
    comingFromZed: {
      title: '从 Zed 迁移',
      body: 'Wu 和 Zed 可以同时安装。它们的设置是分开的，如果想让两边设置一样，把 `settings.json` 和 `keymap.json` 从 `~/.config/zed` 复制到 `~/.config/wu` 就行。'
    },
    raycast: {
      title: 'Raycast',
      body: `在 macOS 上，安装 [Raycast 的 Wu 扩展](${RAYCAST_URL})，就能在 Raycast 里打开最近的项目、新窗口和设置。`
    },
    license: {
      title: '许可证',
      body: 'Wu 采用 GPL-3.0-or-later 许可证发布，标注的部分采用 Apache-2.0。'
    }
  },
  support: {
    title: '支持 Wu',
    intro: 'Wu 免费开源，无需账号，没有遥测。它由一位开发者独立开发和维护。',
    why: '如果你每天都在用 Wu，捐助可以帮忙支付修复问题、添加功能，以及让 Wu 跟上 Zed 更新所花的时间。',
    donate: '在 Open Collective 上捐助',
    donateNote: '付款由 Open Collective 处理，每一笔捐助和支出都会公开。',
    otherWays: {
      title: '其他帮忙的方式',
      body: '在 GitHub 上给 Wu 点个星，报告你遇到的问题，或者推荐给可能喜欢它的朋友。',
      star: '在 GitHub 上点星',
      reportIssue: '报告问题'
    }
  },
  notFound: {
    title: '找不到页面',
    body: '这个页面不存在，或者已经移走了。',
    back: '返回 Wu'
  },
  wuWindow: {
    panels: {
      explorer: '项目面板',
      search: '搜索',
      git: 'Git 面板',
      outline: '大纲面板',
      debug: '调试面板',
      agent: '智能体对话'
    },
    manage: '管理',
    blame: 'farshed，3 周前',
    search: {
      placeholder: '搜索',
      label: '搜索项目',
      matchCase: '区分大小写',
      wholeWords: '全字匹配',
      regex: '使用正则表达式',
      results: (matches: number, files: number) => `${files} 个文件中有 ${matches} 个结果`
    },
    git: {
      changes: '更改',
      history: '历史记录',
      viewDiff: '查看差异',
      viewOptions: '视图选项',
      tracked: '已跟踪',
      commitMessage: '输入提交消息',
      commitTracked: '提交已跟踪的更改'
    },
    outline: { search: '搜索缓冲区符号…', pin: '固定当前大纲' },
    debug: {
      newSession: '新建会话',
      editConfig: '编辑 debug.json',
      docs: '调试器文档',
      extensions: '调试器扩展',
      breakpoints: '断点',
      noBreakpoints: '未设置断点'
    },
    agentPanel: {
      chats: '对话',
      savedMessages: '已收藏的消息',
      backToChats: '返回对话列表',
      searchChats: '搜索对话…',
      searchSaved: '搜索已收藏的消息…',
      noSaved: '还没有收藏的消息。将鼠标悬停在消息上，点击书签图标即可收藏。',
      removeSaved: '取消收藏',
      you: '你',
      now: '刚刚'
    },
    tabs: {
      uncommittedChanges: '未提交的更改',
      newChat: '新对话',
      close: (title: string) => `关闭 ${title}`,
      new: '新建…',
      goBack: '后退',
      goForward: '前进',
      splitPane: '拆分窗格',
      zoomIn: '最大化窗格'
    },
    toolbar: { bufferSearch: '缓冲区搜索', selectionControls: '选区选项', editorControls: '编辑器选项' },
    statusBar: { languageServers: '语言服务器', diagnostics: '项目诊断', terminal: '终端面板' },
    diff: {
      collapseAll: '折叠所有文件',
      unified: '统一',
      split: '拆分',
      previousHunk: '转到上一个差异块',
      nextHunk: '转到下一个差异块',
      stage: '暂存',
      unstage: '取消暂存',
      stageAll: '全部暂存',
      commit: '提交',
      openFile: '打开文件'
    },
    chat: {
      placeholder: '想做什么都行…',
      messageLabel: '消息',
      send: '发送',
      thinking: '思考中…',
      working: '琢磨中…',
      effort: '高',
      checkout: '本地签出',
      saveMessage: '收藏消息',
      justNow: '刚刚',
      thoughtProcess: '思考过程',
      tools: { run: '运行', read: '读取', edit: '编辑', search: '搜索' }
    },
    summary: {
      separator: ' · ',
      thoughtOnce: '思考过程',
      thoughtTimes: (count: number) => `思考了 ${count} 次`,
      ran: (count: number) => `运行了 ${count} 个命令`,
      edited: (count: number) => `编辑了 ${count} 个文件`,
      read: (count: number) => `读取了 ${count} 个文件`,
      searched: (count: number) => `搜索了 ${count} 次`
    },
    chats: [
      {
        id: 'preview-tests',
        title: '为预览标签页添加测试',
        age: '2 分钟',
        entries: [
          { kind: 'user', text: '添加一个测试：关闭预览标签页后，激活的仍是正确的标签页。', time: '10 月 5 日 下午 3:42' },
          {
            kind: 'steps',
            steps: [
              {
                kind: 'thought',
                text: '逻辑在 pane.rs 的 `close_current_preview_item` 里。它会移除预览项，如果关闭的标签页在激活标签页之前，就把 `active_item_index` 减一。好的测试应该打开三个标签页，把第一个设为预览，激活最后一个，关闭预览，再检查哪个标签页处于激活状态。'
              },
              { kind: 'tool', tool: 'read', detail: 'crates/workspace/src/pane.rs' },
              { kind: 'tool', tool: 'search', detail: 'fn add_labeled_item in crates/workspace' },
              { kind: 'tool', tool: 'read', detail: 'crates/workspace/src/pane/tests.rs' },
              { kind: 'tool', tool: 'edit', detail: 'crates/workspace/src/pane/tests.rs', added: 21 },
              { kind: 'tool', tool: 'run', detail: 'cargo test -p workspace preview' }
            ]
          },
          {
            kind: 'assistant',
            text: '在窗格测试里添加了 `closing_preview_tab_keeps_active_tab`。它打开三个标签页，把第一个设为预览，激活最后一个，然后关闭预览：\n\n```rust\n#[gpui::test]\nasync fn closing_preview_tab_keeps_active_tab(cx: &mut TestAppContext) {\n    let (pane, cx) = init_pane(cx).await;\n    add_labeled_item(&pane, "A", false, cx);\n    add_labeled_item(&pane, "B", false, cx);\n    add_labeled_item(&pane, "C", false, cx);\n    set_preview_item(&pane, "A", cx);\n\n    pane.update_in(cx, |pane, window, cx| {\n        pane.activate_item(2, false, false, window, cx);\n        pane.close_current_preview_item(window, cx);\n    });\n\n    assert_item_labels(&pane, ["B", "C*"], cx);\n}\n```\n\n`*` 表示激活的标签页，所以这个测试检查的是 A 关掉之后 C 仍然处于激活状态。测试通过了，`workspace` 里另外 213 个测试也都通过。'
          },
          { kind: 'user', text: '再覆盖一下预览标签页在激活标签页之后的情况。', time: '10 月 5 日 下午 3:45' },
          {
            kind: 'steps',
            steps: [
              {
                kind: 'thought',
                text: '预览标签页在激活标签页之后时，`item_idx < prev_active_item_index` 为 false，所以索引应该保持不变。设置一样，只是把预览放到最后，让第一个标签页保持激活。'
              },
              { kind: 'tool', tool: 'edit', detail: 'crates/workspace/src/pane/tests.rs', added: 17 },
              { kind: 'tool', tool: 'run', detail: 'cargo test -p workspace preview' }
            ]
          },
          {
            kind: 'assistant',
            text: '添加了 `closing_later_preview_tab_keeps_active_tab`。现在两种情况都通过了：\n\n- **预览在激活标签页之前：**激活索引减一，所以激活的还是同一个标签页。\n- **预览在激活标签页之后：**激活索引保持不变。\n\n两个测试都放在现有的预览测试旁边，方便查找。'
          }
        ]
      },
      {
        id: 'pane-groups',
        title: '窗格拆分是怎么实现的？',
        age: '1 小时',
        entries: [
          { kind: 'user', text: '拆分窗格是怎么实现的？', time: '10 月 5 日 下午 2:10' },
          {
            kind: 'steps',
            steps: [
              { kind: 'tool', tool: 'read', detail: 'crates/workspace/src/pane_group.rs' },
              { kind: 'tool', tool: 'search', detail: 'fn new_axis in crates/workspace' }
            ]
          },
          {
            kind: 'assistant',
            text: '每个窗口的中间区域是一个 `PaneGroup`。它的 `root` 是一棵树：每个节点要么是单个 `Pane`，要么是一个 `Axis`，里面横向或纵向排列着更多成员。\n\n```rust\npub enum Member {\n    Axis(PaneAxis),\n    Pane(Entity<Pane>),\n}\n```\n\n- `split` 找到你要拆分的窗格，把它换成一个新的轴，里面放着原来的窗格和新窗格。\n- 如果父轴的方向已经相同，新窗格会直接加到父轴里，不会再嵌套一层轴。\n- `panes` 遍历这棵树，按从左到右、从上到下的顺序返回所有窗格。'
          },
          { kind: 'user', text: '拆分方向是从哪里来的？', time: '10 月 5 日 下午 2:12' },
          {
            kind: 'steps',
            steps: [
              { kind: 'tool', tool: 'search', detail: 'SplitDirection in crates/workspace' },
              { kind: 'tool', tool: 'read', detail: 'crates/workspace/src/workspace.rs' }
            ]
          },
          {
            kind: 'assistant',
            text: '来自你执行的操作。`SplitRight`、`SplitLeft`、`SplitUp` 和 `SplitDown` 各自对应一个 `SplitDirection`，`Workspace::split_pane` 再把它传给窗格组。\n\n把标签页拖到窗格边缘也是一样：窗格会判断你放在了哪条边上，然后据此选择方向。'
          }
        ]
      },
      {
        id: 'background-tasks',
        title: '跟踪后台命令',
        age: '1 天',
        entries: [
          { kind: 'user', text: '记录哪些后台命令还在运行。', time: '10 月 4 日 下午 6:05' },
          {
            kind: 'steps',
            steps: [
              {
                kind: 'thought',
                text: '每当运行中的任务有变化，命令行工具都会发来完整的列表。会话可以每次替换自己的列表，同时保留已知任务的开始时间。'
              },
              { kind: 'tool', tool: 'read', detail: 'crates/agent_chat/src/session.rs' },
              { kind: 'tool', tool: 'edit', detail: 'crates/agent_chat/src/session.rs', added: 41, removed: 6 },
              { kind: 'tool', tool: 'run', detail: 'cargo test -p agent_chat background' }
            ]
          },
          {
            kind: 'assistant',
            text: '现在会话会保存一份运行中的后台任务列表。`background_tasks()` 会连同每个任务的开始时间一起返回，这样对话里就能显示每个任务已经运行了多久。\n\n一轮运行结束时列表会被清空，所以停下的智能体不会在屏幕上留下卡住的任务。'
          },
          { kind: 'user', text: '把它们显示在输入框下方，并显示每个任务已经运行了多久。', time: '10 月 4 日 下午 6:20' },
          {
            kind: 'steps',
            steps: [
              { kind: 'tool', tool: 'read', detail: 'crates/agent_chat/src/chat_view/composer.rs' },
              { kind: 'tool', tool: 'edit', detail: 'crates/agent_chat/src/chat_view/background_tasks.rs', added: 118 },
              { kind: 'tool', tool: 'edit', detail: 'crates/agent_chat/src/chat_view/composer.rs', added: 1 },
              { kind: 'tool', tool: 'run', detail: 'cargo test -p agent_chat background' }
            ]
          },
          {
            kind: 'assistant',
            text: '现在只要有任务在后台运行，输入框下方的栏里就会显示一个小标签：\n\n- **一条命令：**显示“命令运行中”和已运行的时间。\n- **多个任务：**显示“3 个任务运行中”。\n- **点击它**可以查看每条命令，或者在单独的标签页里打开子智能体。\n\n时间每秒更新一次，最后一个任务结束时就会停止。'
          }
        ]
      }
    ] as MockChat[],
    previewReply:
      '这只是一个预览，后面没有真正的智能体。在 Wu 里，Claude Code 或 Codex 会直接在这里回答，用的是你已经登录的命令行工具。'
  },
  agentWindow: {
    message: '网络错误导致上传失败时自动重试',
    chatTitle: '网络错误时重试上传',
    steps: [
      {
        kind: 'thought',
        text: '上传都经过 `UploadClient::send`。只有网络错误值得重试，所以重试前应该先检查错误类型。'
      },
      { kind: 'tool', tool: 'search', detail: 'fn send in src/upload' },
      { kind: 'tool', tool: 'read', detail: 'src/upload/client.rs' },
      { kind: 'tool', tool: 'edit', detail: 'src/upload/client.rs', added: 31, removed: 4 },
      { kind: 'tool', tool: 'run', detail: 'cargo test -p upload' }
    ] as ChatStep[],
    reply:
      '现在网络断开时，上传最多会重试 3 次，每次重试前等待的时间稍长一点。其他错误，比如文件被拒绝，仍然会立即失败，因为重试也没用。\n\n为每种情况都添加了测试，`upload` 里的 26 个测试全部通过。'
  }
};
