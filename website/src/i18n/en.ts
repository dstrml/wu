import type { ChatStep, MockChat } from '../components/wuWindowData';
import {
  INSTALL_COMMAND,
  INSTALL_GUIDE_URL,
  ISSUES_URL,
  RAYCAST_URL,
  SITE_DESCRIPTION,
  ZED_DOCS_URL
} from '../consts';

const plural = (amount: number, one: string, many: string) => `${amount} ${amount === 1 ? one : many}`;

export const en = {
  meta: {
    home: { title: 'Wu: the fast, native code editor and agent workspace', description: SITE_DESCRIPTION },
    docs: { title: 'Wu Docs', description: 'How Wu differs from Zed: what is removed, what is different, and where things live.' },
    support: { title: 'Support Wu', description: 'Wu is free and open source. Donate on Open Collective to help keep it going.' },
    notFound: { title: 'Page not found · Wu' }
  },
  languageName: 'English',
  header: {
    docs: 'Docs',
    support: 'Support',
    download: 'Download'
  },
  footer: {
    navLabel: 'Footer',
    product: { title: 'Wu', download: 'Download', docs: 'Docs', releases: 'Releases' },
    resources: {
      title: 'Resources',
      installGuide: 'Install guide',
      memoryBenchmark: 'Memory benchmark',
      raycast: 'Raycast extension'
    },
    source: { title: 'Source', github: 'GitHub', reportIssue: 'Report an issue', support: 'Support Wu' },
    license: 'Wu is licensed under GPL-3.0-or-later.'
  },
  installCommand: { copied: 'Copied', copy: 'Copy', copyLabel: 'Copy install command' },
  downloads: {
    macArm: 'macOS (Apple Silicon)',
    macIntel: 'macOS (Intel)',
    windows: 'Windows (x86-64)',
    linuxX86: 'Linux (x86-64)',
    linuxArm: 'Linux (AArch64)'
  },
  home: {
    title: 'The fast, native code editor and agent workspace',
    tagline: 'Written in Rust. Batteries included: coding agents, a delightful UX and everything you need from day one.',
    installManually: (href: string) => `Or [install manually](${href})`,
    downloadFor: (platform: string) => `Download for ${platform}`,
    downloadWu: 'Download Wu',
    allPlatforms: 'All platforms',
    memory: {
      title: 'Light on memory',
      body: 'Wu used less memory than Zed and VS Code in every test we ran.',
      link: 'See benchmarks'
    },
    agents: {
      title: 'Your coding agents, built in',
      body: "Work with Claude Code, Codex and OpenCode right next to your code, using the existing accounts you're signed in from.",
      link: 'How it works'
    },
    openSource: {
      title: 'Open source, built on Zed',
      body: "Wu is a fork of Zed. It keeps Zed's editor core, GPU rendering and language tooling, and every Zed extension works in Wu.",
      source: 'Check out the source',
      docs: 'Read the docs'
    },
    ready: {
      iconAlt: 'Wu app icon',
      title: 'Ready when you are',
      bodyScript: 'Wu is free and takes a minute to install. Run this in your terminal.',
      bodyPick: 'Wu is free and takes a minute to install. Pick your platform.',
      orInstallManually: 'Or install manually',
      alsoRun: 'On macOS or Linux, you can also run',
      firstTime: `First time on macOS or Linux? See the [install guide](${INSTALL_GUIDE_URL}).`
    }
  },
  memoryChart: {
    label: 'Memory use in MiB, lower is better',
    caption: 'Total process memory, lower is better.',
    scenarios: { idle: 'Sitting idle', search: 'Searching a repository' }
  },
  faq: {
    title: 'Questions people ask about Wu',
    questions: [
      {
        question: 'Is Wu free?',
        answer: "Yes. Wu is free and open source under GPL-3.0-or-later, the same license as Zed. There's no paid tier."
      },
      {
        question: 'Does Wu collect any data?',
        answer: 'No. Wu has no telemetry, no crash reporting and no account. It never sends your usage data anywhere.'
      },
      {
        question: 'Does Wu support AI coding agents?',
        answer:
          "Yes. Wu has built-in support for coding agents like Claude Code, Codex, and OpenCode. However, Wu doesn't support Zed's AI features, like the inline assistant and edit predictions."
      },
      {
        question: 'Do Zed extensions work in Wu?',
        answer: 'Yes. Wu installs extensions from the Zed extension registry, and every Zed extension works.'
      },
      {
        question: 'Can I keep Zed installed too?',
        answer:
          'Yes. Wu and Zed keep separate settings, in `~/.config/wu` and `~/.config/zed`. Copy your `settings.json` and `keymap.json` over if you want the same setup in both.'
      },
      {
        question: "Why does macOS say Wu can't be opened?",
        answer: `Wu isn't signed with an Apple Developer certificate yet, so macOS blocks it the first time. The [install guide](${INSTALL_GUIDE_URL}) shows the one-time fix.`
      },
      {
        question: 'How is Wu different from Zed?',
        answer:
          "Wu drops Zed's AI service, collaboration and telemetry features. It has its own agent chat built on the Claude Code, Codex and OpenCode CLIs, its own look, and several other improvements over Zed. See [docs](/docs/) for more."
      },
      {
        question: 'Where can I report a problem?',
        answer: `Open an issue on [GitHub](${ISSUES_URL}). Please check for an existing one first.`
      }
    ]
  },
  docs: {
    title: 'Docs',
    intro: `This page covers what is specific to Wu. Wu shares its editor core with Zed, so for settings, key bindings, languages, extensions, tasks, debugging, and remote development, the [Zed documentation](${ZED_DOCS_URL}) applies to Wu as well.`,
    gettingStarted: {
      title: 'Getting started',
      body: `On macOS and Linux, run \`${INSTALL_COMMAND}\` in your terminal. Or download Wu for your platform and follow the [install guide](${INSTALL_GUIDE_URL}).`,
      paths: {
        commandLine: 'Command line',
        settingsUnix: 'Settings (macOS, Linux)',
        settingsWindows: 'Settings (Windows)',
        appDataMac: 'App data (macOS)',
        projectSettings: 'Project settings'
      }
    },
    agentChat: {
      title: 'Agent Chat',
      intro:
        'Chat with Claude Code, Codex or OpenCode inside Wu. It uses the command line tools you already have installed and signed in, so there are no API keys and no Wu account.',
      items: [
        '**Start a chat** from the agent icon in the activity bar. Continue Saved Chat picks up one you started in the terminal.',
        '**Type** `/` for commands, `@` to mention a file and `$` for a skill. Paste or drop images to attach them.',
        '**Send while it works** and your message waits in a queue. You can edit it or use it to steer the current turn.',
        '**Dictate** with `Cmd-D` (`Ctrl-D` on Windows and Linux). It runs on your machine and needs macOS 14 or later with Apple Silicon, Linux or Windows.',
        '**The model picker** sets the model, effort and permissions. The rings next to it show context and plan usage.',
        '**Chats** can be pinned, forked and renamed. Subagents open in their own tabs.'
      ],
      outro:
        'Notifications, auto-compact, Claude in Chrome, hiding agents and turning Agent Chat off are on the Agent Chat page in settings.'
    },
    workspace: {
      title: 'Workspace',
      items: [
        'The activity bar on the side switches between the project, git, outline, search, debug, and agent panels. The panels dock on the left by default, and `activity_bar.icon_size` sets the icon size.',
        'The `+` button in the editor and terminal tab strips sits after the last tab and opens a new file or terminal directly, with no menu.',
        '`Cmd-W` / `Ctrl-W` only closes editor tabs, never a side panel.'
      ]
    },
    extensions: {
      title: 'Extensions',
      body: 'Wu installs extensions from the Zed extension registry, and they work in Wu without changes.'
    },
    privacy: {
      title: 'Privacy',
      body: 'Wu has no account, no telemetry, and no crash reporting. It never sends your usage data anywhere. Agent Chat only talks to the providers behind the tools you use.'
    },
    notIncluded: {
      title: 'Not included',
      items: [
        'Collaboration: channels, calls, screen sharing, and shared projects.',
        'Vim and Helix modes.',
        'Dev containers.',
        'The REPL and the journal.'
      ],
      note: 'If a setting or key binding in the Zed docs belongs to one of these, it has no effect in Wu.'
    },
    comingFromZed: {
      title: 'Coming from Zed',
      body: 'Wu and Zed can be installed side by side. They keep separate settings, so copy your `settings.json` and `keymap.json` from `~/.config/zed` to `~/.config/wu` if you want the same setup in both.'
    },
    raycast: {
      title: 'Raycast',
      body: `On macOS, install the [Wu extension for Raycast](${RAYCAST_URL}) to open recent projects, new windows, and settings from Raycast.`
    },
    license: {
      title: 'License',
      body: 'Wu is distributed under GPL-3.0-or-later, with Apache-2.0 components where marked.'
    }
  },
  support: {
    title: 'Support Wu',
    intro: "Wu is free and open source, with no account and no telemetry. It's built and maintained by one developer.",
    why: 'If Wu is part of your day, a donation helps pay for the time it takes to fix bugs, add features and keep Wu up to date with Zed.',
    donate: 'Donate on Open Collective',
    donateNote: 'Open Collective handles the payment and shows every donation and expense in public.',
    otherWays: {
      title: 'Other ways to help',
      body: 'Star Wu on GitHub, report bugs you run into, and tell a friend who might like it.',
      star: 'Star on GitHub',
      reportIssue: 'Report an issue'
    }
  },
  notFound: {
    title: 'Page not found',
    body: "This page doesn't exist or has moved.",
    back: 'Back to Wu'
  },
  wuWindow: {
    panels: {
      explorer: 'Project Panel',
      search: 'Search',
      git: 'Git Panel',
      outline: 'Outline Panel',
      debug: 'Debug Panel',
      agent: 'Agent Chats'
    },
    manage: 'Manage',
    blame: 'farshed, 3 weeks ago',
    search: {
      placeholder: 'Search',
      label: 'Search the project',
      matchCase: 'Match Case',
      wholeWords: 'Match Whole Words',
      regex: 'Use Regular Expressions',
      results: (matches: number, files: number) =>
        `${plural(matches, 'result', 'results')} in ${plural(files, 'file', 'files')}`
    },
    git: {
      changes: 'Changes',
      history: 'History',
      viewDiff: 'View Diff',
      viewOptions: 'View Options',
      tracked: 'Tracked',
      commitMessage: 'Enter commit message',
      commitTracked: 'Commit Tracked'
    },
    outline: { search: 'Search buffer symbols…', pin: 'Pin Active Outline' },
    debug: {
      newSession: 'New Session',
      editConfig: 'Edit debug.json',
      docs: 'Debugger Docs',
      extensions: 'Debugger Extensions',
      breakpoints: 'Breakpoints',
      noBreakpoints: 'No Breakpoints Set'
    },
    agentPanel: {
      chats: 'Chats',
      savedMessages: 'Saved Messages',
      backToChats: 'Back to Chats',
      searchChats: 'Search chats…',
      searchSaved: 'Search saved messages…',
      noSaved: 'No saved messages yet. Hover a message and click the bookmark to save it.',
      removeSaved: 'Remove from saved',
      you: 'You',
      now: 'now'
    },
    tabs: {
      uncommittedChanges: 'Uncommitted Changes',
      newChat: 'New chat',
      close: (title: string) => `Close ${title}`,
      new: 'New...',
      goBack: 'Go Back',
      goForward: 'Go Forward',
      splitPane: 'Split Pane',
      zoomIn: 'Zoom In'
    },
    toolbar: { bufferSearch: 'Buffer Search', selectionControls: 'Selection Controls', editorControls: 'Editor Controls' },
    statusBar: { languageServers: 'Language Servers', diagnostics: 'Project Diagnostics', terminal: 'Terminal Panel' },
    diff: {
      collapseAll: 'Collapse All Files',
      unified: 'Unified',
      split: 'Split',
      previousHunk: 'Go to Previous Hunk',
      nextHunk: 'Go to Next Hunk',
      stage: 'Stage',
      unstage: 'Unstage',
      stageAll: 'Stage All',
      commit: 'Commit',
      openFile: 'Open File'
    },
    chat: {
      placeholder: 'Do anything…',
      messageLabel: 'Message',
      send: 'Send',
      thinking: 'Thinking…',
      working: 'Pondering…',
      effort: 'High',
      checkout: 'Local checkout',
      saveMessage: 'Save message',
      justNow: 'Just now',
      thoughtProcess: 'Thought process',
      tools: { run: 'Run', read: 'Read', edit: 'Edit', search: 'Search' }
    },
    summary: {
      separator: ' · ',
      thoughtOnce: 'thought process',
      thoughtTimes: (count: number) => `thought ${count} times`,
      ran: (count: number) => `ran ${plural(count, 'command', 'commands')}`,
      edited: (count: number) => `edited ${plural(count, 'file', 'files')}`,
      read: (count: number) => `read ${plural(count, 'file', 'files')}`,
      searched: (count: number) => `searched ${plural(count, 'time', 'times')}`
    },
    chats: [
      {
        id: 'preview-tests',
        title: 'Add a test for preview tabs',
        age: '2m',
        entries: [
          { kind: 'user', text: 'Add a test that closing the preview tab keeps the right tab active.', time: 'Oct 5, 3:42 PM' },
          {
            kind: 'steps',
            steps: [
              {
                kind: 'thought',
                text: 'The logic lives in `close_current_preview_item` in pane.rs. It removes the preview item and moves `active_item_index` back by one when the closed tab sat before the active one. A good test opens three tabs, makes the first one a preview, activates the last one, closes the preview and checks which tab is active.'
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
            text: 'Added `closing_preview_tab_keeps_active_tab` to the pane tests. It opens three tabs, makes the first one a preview, activates the last one and closes the preview:\n\n```rust\n#[gpui::test]\nasync fn closing_preview_tab_keeps_active_tab(cx: &mut TestAppContext) {\n    let (pane, cx) = init_pane(cx).await;\n    add_labeled_item(&pane, "A", false, cx);\n    add_labeled_item(&pane, "B", false, cx);\n    add_labeled_item(&pane, "C", false, cx);\n    set_preview_item(&pane, "A", cx);\n\n    pane.update_in(cx, |pane, window, cx| {\n        pane.activate_item(2, false, false, window, cx);\n        pane.close_current_preview_item(window, cx);\n    });\n\n    assert_item_labels(&pane, ["B", "C*"], cx);\n}\n```\n\nThe `*` marks the active tab, so the test checks that C is still active after A goes away. It passes, along with the other 213 tests in `workspace`.'
          },
          { kind: 'user', text: 'Also cover the case where the preview tab is after the active one.', time: 'Oct 5, 3:45 PM' },
          {
            kind: 'steps',
            steps: [
              {
                kind: 'thought',
                text: 'When the preview tab sits after the active one, `item_idx < prev_active_item_index` is false, so the index must stay the same. Same setup, but the preview goes last and the first tab stays active.'
              },
              { kind: 'tool', tool: 'edit', detail: 'crates/workspace/src/pane/tests.rs', added: 17 },
              { kind: 'tool', tool: 'run', detail: 'cargo test -p workspace preview' }
            ]
          },
          {
            kind: 'assistant',
            text: 'Added `closing_later_preview_tab_keeps_active_tab`. Both cases now pass:\n\n- **Preview before the active tab:** the active index moves back by one, so the same tab stays active.\n- **Preview after the active tab:** the active index stays where it is.\n\nI kept both tests next to the existing preview tests so they are easy to find.'
          }
        ]
      },
      {
        id: 'pane-groups',
        title: 'How do pane splits work?',
        age: '1h',
        entries: [
          { kind: 'user', text: 'How does splitting a pane work?', time: 'Oct 5, 2:10 PM' },
          {
            kind: 'steps',
            steps: [
              { kind: 'tool', tool: 'read', detail: 'crates/workspace/src/pane_group.rs' },
              { kind: 'tool', tool: 'search', detail: 'fn new_axis in crates/workspace' }
            ]
          },
          {
            kind: 'assistant',
            text: "The center of each window is a `PaneGroup`. Its `root` is a tree: each node is either a single `Pane` or an `Axis` that holds more members side by side or stacked.\n\n```rust\npub enum Member {\n    Axis(PaneAxis),\n    Pane(Entity<Pane>),\n}\n```\n\n- `split` finds the pane you're splitting and swaps it for a new axis that holds the old pane and the new one.\n- If the parent axis already runs in the same direction, the new pane is added to it instead of nesting another axis.\n- `panes` walks the tree and returns every pane, left to right and top to bottom."
          },
          { kind: 'user', text: 'Where does the split direction come from?', time: 'Oct 5, 2:12 PM' },
          {
            kind: 'steps',
            steps: [
              { kind: 'tool', tool: 'search', detail: 'SplitDirection in crates/workspace' },
              { kind: 'tool', tool: 'read', detail: 'crates/workspace/src/workspace.rs' }
            ]
          },
          {
            kind: 'assistant',
            text: 'From the action you run. `SplitRight`, `SplitLeft`, `SplitUp` and `SplitDown` each map to a `SplitDirection`, and `Workspace::split_pane` passes it on to the pane group.\n\nDragging a tab to the edge of a pane does the same thing: the pane works out which edge you dropped on and picks the direction from that.'
          }
        ]
      },
      {
        id: 'background-tasks',
        title: 'Track background commands',
        age: '1d',
        entries: [
          { kind: 'user', text: 'Keep track of which background commands are still running.', time: 'Oct 4, 6:05 PM' },
          {
            kind: 'steps',
            steps: [
              {
                kind: 'thought',
                text: 'The CLI sends the full list of running tasks whenever it changes. The session can replace its list each time and keep the start time of tasks it already knew about.'
              },
              { kind: 'tool', tool: 'read', detail: 'crates/agent_chat/src/session.rs' },
              { kind: 'tool', tool: 'edit', detail: 'crates/agent_chat/src/session.rs', added: 41, removed: 6 },
              { kind: 'tool', tool: 'run', detail: 'cargo test -p agent_chat background' }
            ]
          },
          {
            kind: 'assistant',
            text: 'Sessions now keep a list of running background tasks. `background_tasks()` returns them with the time each one started, so the chat can show how long each one has been running.\n\nWhen a run ends, the list is cleared, so a stopped agent never leaves a task stuck on screen.'
          },
          { kind: 'user', text: 'Show them under the chat box, with how long each one has been running.', time: 'Oct 4, 6:20 PM' },
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
            text: 'The bar under the chat box now shows a small pill while anything runs in the background:\n\n- **One command:** "Command running" with the elapsed time.\n- **Several:** "3 tasks running".\n- **Click it** to see each command, or to open a subagent in its own tab.\n\nThe time updates once a second and stops as soon as the last task ends.'
          }
        ]
      }
    ] as MockChat[],
    previewReply:
      "This is only a preview, so there's no agent behind it. In Wu, Claude Code or Codex answers right here, using the CLI you already have signed in."
  },
  agentWindow: {
    message: 'Retry uploads that fail because of a network error',
    chatTitle: 'Retry uploads on network errors',
    steps: [
      {
        kind: 'thought',
        text: 'Uploads go through `UploadClient::send`. Only network errors are worth retrying, so the retry should check the error kind first.'
      },
      { kind: 'tool', tool: 'search', detail: 'fn send in src/upload' },
      { kind: 'tool', tool: 'read', detail: 'src/upload/client.rs' },
      { kind: 'tool', tool: 'edit', detail: 'src/upload/client.rs', added: 31, removed: 4 },
      { kind: 'tool', tool: 'run', detail: 'cargo test -p upload' }
    ] as ChatStep[],
    reply:
      "Uploads now retry up to 3 times when the network drops, waiting a little longer before each try. Other errors, like a rejected file, still fail right away since retrying won't help.\n\nAdded a test for each case, and all 26 tests in `upload` pass."
  }
};

export type Messages = typeof en;
