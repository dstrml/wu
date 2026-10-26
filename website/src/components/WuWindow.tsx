import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import type { Messages } from '../i18n/en';
import { useMessages } from '../i18n/locale';
import {
  fileByPath,
  fileIcon,
  fileName,
  files,
  folderIcon,
  highlight,
  initiallyExpanded,
  languageName,
  outline,
  tree,
  type ChatEntry,
  type ChatStep,
  type MockFile,
  type TokenKind,
  type ToolKind,
  type TreeNode
} from './wuWindowData';

type PanelKey = 'explorer' | 'search' | 'git' | 'outline' | 'debug' | 'agent';
type TabId = `file:${string}` | `chat:${string}` | 'diff';

const WIDTH = 1440;
const HEIGHT = 900;
const LINE_HEIGHT = 24;
const PANEL_WIDTH = 240;
const CHAR_WIDTH = 9;
const GUTTER_ICON_AREA = 26;
const GUTTER_FOLD_AREA = 26;

export const text = '#f0eef4';
export const muted = '#aaa8b7';
export const placeholder = '#727080';
export const accent = '#7aa2f7';
export const claude = '#d97757';

const tokenStyles: Record<TokenKind, CSSProperties> = {
  keyword: { color: '#8b7cf6' },
  type: { color: '#c084fc' },
  function: { color: '#60a5fa' },
  string: { color: '#34d399' },
  number: { color: '#facc15' },
  comment: { color: '#92929a', fontStyle: 'italic' },
  property: { color: '#f472b6' },
  punctuation: { color: '#a1a1aa' },
  self: { color: '#8b7cf6' },
  constant: { color: '#facc15' },
  attribute: { color: '#22d3ee' },
  title: { color: '#8b7cf6' },
  emphasis: { color: '#facc15' },
  link: { color: '#60a5fa', fontStyle: 'italic' },
  marker: { color: '#f472b6' },
  literal: { color: '#34d399' }
};

const panels: { key: PanelKey; icon: string }[] = [
  { key: 'explorer', icon: 'activity_explorer' },
  { key: 'search', icon: 'activity_search' },
  { key: 'git', icon: 'activity_source_control' },
  { key: 'outline', icon: 'activity_outline' },
  { key: 'debug', icon: 'activity_debug' },
  { key: 'agent', icon: 'agent_bot' }
];

const toolIcons: Record<ToolKind, string> = {
  run: 'agent_terminal',
  read: 'agent_document',
  edit: 'agent_pen',
  search: 'agent_magnifer'
};

const changedFiles = files.filter((file) => file.modified);
export const hover = 'hover:bg-[#ffffff1c]';

export function Icon({ name, size = 16, color, className = '' }: { name: string; size?: number; color?: string; className?: string }) {
  const mask = `url(/wu-ui/${name}.svg) center / contain no-repeat`;
  return (
    <span
      aria-hidden="true"
      className={`inline-block shrink-0 bg-current ${className}`}
      style={{ width: size, height: size, mask, WebkitMask: mask, color }}
    />
  );
}

function ImageIcon({ src, size = 16, className = '' }: { src: string; size?: number; className?: string }) {
  return <img src={src} alt="" width={size} height={size} className={`shrink-0 ${className}`} draggable={false} />;
}

function IconButton({
  name,
  size = 14,
  box = 22,
  color = text,
  label
}: {
  name: string;
  size?: number;
  box?: number;
  color?: string;
  label: string;
}) {
  return (
    <span title={label} className={`grid shrink-0 place-items-center rounded-[4px] ${hover}`} style={{ width: box, height: box }}>
      <Icon name={name} size={size} color={color} />
    </span>
  );
}

function Highlighted({ code, language }: { code: string; language: MockFile['language'] }) {
  return (
    <>
      {highlight(code, language).map((token, index) => (
        <span key={index} style={token.kind ? tokenStyles[token.kind] : undefined}>
          {token.text}
        </span>
      ))}
    </>
  );
}

function InlineMarkdown({ source, codeClassName }: { source: string; codeClassName: string }) {
  return (
    <>
      {source.split(/(`[^`]+`|\*\*[^*]+\*\*)/).map((part, index) => {
        if (part.startsWith('`') && part.endsWith('`')) {
          return (
            <code key={index} className={codeClassName}>
              {part.slice(1, -1)}
            </code>
          );
        }
        if (part.startsWith('**') && part.endsWith('**')) {
          return (
            <strong key={index} className="font-semibold">
              {part.slice(2, -2)}
            </strong>
          );
        }
        return <span key={index}>{part}</span>;
      })}
    </>
  );
}

const inlineCode = 'rounded-[4px] bg-[#7aa2f71f] px-px font-wu-mono text-[#7aa2f7]';

function CodeBlock({ language, code }: { language: string; code: string }) {
  return (
    <div className="overflow-hidden rounded-[10px] border border-[#2c2f3a] bg-[#ffffff09]">
      <div className="flex h-7 items-center border-b border-[#2c2f3a] bg-[#ffffff05] pr-[5px] pl-3 text-[11px]" style={{ color: muted }}>
        {language}
        <span className={`ml-auto flex h-[22px] items-center rounded-[5px] px-1.5 ${hover}`}>
          <Icon name="agent_copy" size={12} />
        </span>
      </div>
      <pre className="overflow-x-auto px-3 py-2.5 font-wu-mono text-[12.5px] leading-[18px]" style={{ color: text }}>
        {code.split('\n').map((line, index) => (
          <div key={index}>{language === 'rust' ? <Highlighted code={line} language="rust" /> : line || ' '}</div>
        ))}
      </pre>
    </div>
  );
}

export function AssistantText({ source }: { source: string }) {
  const blocks = source.split(/\n?```(\w*)\n([\s\S]*?)\n```\n?/);
  const elements: ReactNode[] = [];
  for (let index = 0; index < blocks.length; index += 3) {
    const prose = blocks[index] ?? '';
    prose
      .split('\n\n')
      .filter((block) => block.trim())
      .forEach((block, blockIndex) => {
        const lines = block.split('\n');
        const key = `${index}:${blockIndex}`;
        if (lines.every((line) => line.startsWith('- '))) {
          elements.push(
            <ul key={key} className="flex list-disc flex-col gap-1 pl-5">
              {lines.map((line) => (
                <li key={line}>
                  <InlineMarkdown source={line.slice(2)} codeClassName={inlineCode} />
                </li>
              ))}
            </ul>
          );
        } else {
          elements.push(
            <p key={key}>
              <InlineMarkdown source={block} codeClassName={inlineCode} />
            </p>
          );
        }
      });
    const code = blocks[index + 2];
    if (code !== undefined) elements.push(<CodeBlock key={`${index}:code`} language={blocks[index + 1] ?? ''} code={code} />);
  }
  return (
    <div className="flex flex-col gap-3 text-[14px] leading-[22px]" style={{ color: text }}>
      {elements}
    </div>
  );
}

function indentLevels(lines: string[]) {
  const levels = lines.map((line) => (line.trim() ? Math.floor((line.length - line.trimStart().length) / 4) : -1));
  return levels.map((level, index) => {
    if (level >= 0) return level;
    const before = levels.slice(0, index).findLast((value) => value >= 0) ?? 0;
    const after = levels.slice(index + 1).find((value) => value >= 0) ?? 0;
    return Math.min(before, after);
  });
}

function symbolPath(file: MockFile, line: number) {
  const path: string[] = [];
  for (const item of outline(file)) {
    if (item.line > line) break;
    path.length = item.depth;
    path[item.depth] = item.label;
  }
  return path.filter(Boolean);
}

function Editor({
  file,
  cursor,
  revealToken,
  onCursor
}: {
  file: MockFile;
  cursor: { line: number; column: number };
  revealToken: number;
  onCursor: (line: number, column: number) => void;
}) {
  const t = useMessages().wuWindow;
  const scrollRef = useRef<HTMLDivElement>(null);
  const lines = file.code.split('\n');
  const levels = indentLevels(lines);
  const digits = Math.max(3, String(file.firstLine + lines.length - 1).length);
  const gutterWidth = GUTTER_ICON_AREA + digits * CHAR_WIDTH + GUTTER_FOLD_AREA;
  const hunkColor = (index: number) => {
    const hunk = file.hunks?.find((candidate) => index >= candidate.start && index <= candidate.end);
    if (!hunk) return undefined;
    return hunk.kind === 'added' ? '#34d399' : '#facc15';
  };

  useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    const top = cursor.line * LINE_HEIGHT;
    if (top < scroller.scrollTop || top > scroller.scrollTop + scroller.clientHeight - LINE_HEIGHT * 3) {
      scroller.scrollTop = Math.max(0, top - scroller.clientHeight / 3);
    }
    // Clicking a line moves the cursor too, and that shouldn't scroll.
  }, [revealToken, file.path]);

  return (
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto overscroll-contain bg-[#181a21] font-wu-mono text-[15px]">
      {lines.map((line, index) => {
        const isActive = index === cursor.line;
        const marker = hunkColor(index);
        return (
          <div
            key={index}
            className={`relative flex cursor-text ${isActive ? 'bg-[#ffffff08]' : ''}`}
            style={{ height: LINE_HEIGHT, lineHeight: `${LINE_HEIGHT}px` }}
            onMouseDown={() => onCursor(index, line.length + 1)}
          >
            {marker && <span className="absolute inset-y-0 left-0 w-[6px]" style={{ background: marker }} />}
            <span
              className="shrink-0 text-right select-none"
              style={{ width: gutterWidth, paddingRight: GUTTER_FOLD_AREA, color: isActive ? text : '#555766' }}
            >
              {file.firstLine + index}
            </span>
            <span className="relative whitespace-pre" style={{ color: text }}>
              {Array.from({ length: levels[index] ?? 0 }, (_, level) => (
                <span
                  key={level}
                  aria-hidden="true"
                  className="absolute inset-y-0 w-px bg-[#ffffff0f]"
                  style={{ left: `${level * 4}ch` }}
                />
              ))}
              <Highlighted code={line} language={file.language} />
              {isActive && (
                <span className="inline-block h-6 w-[2px] animate-blink align-top motion-reduce:animate-none" style={{ background: text }} />
              )}
            </span>
            {isActive && line.trim() && (
              <span className="flex items-center gap-2 pl-[61px] whitespace-pre select-none" style={{ color: muted }}>
                <Icon name="file_git" size={17} />
                {t.blame}
              </span>
            )}
          </div>
        );
      })}
      <div style={{ height: LINE_HEIGHT * 8 }} />
    </div>
  );
}

function TreeGutter({ isLast }: { isLast: boolean }) {
  return (
    <svg aria-hidden="true" className="absolute top-0 left-0" width="48" height="32" fill="none" stroke="#ffffff1f" strokeWidth="1">
      <path d={`M12.5 0V${isLast ? 10 : 32}`} />
      <path d="M12.5 10a6 6 0 0 0 6 6H28" />
    </svg>
  );
}

export function StepRow({
  step,
  isLast,
  expanded,
  onToggle
}: {
  step: ChatStep;
  isLast: boolean;
  expanded: boolean;
  onToggle: () => void;
}) {
  const t = useMessages().wuWindow.chat;
  const isThought = step.kind === 'thought';
  const icon = isThought ? 'agent_chat_round_line' : toolIcons[step.tool];
  const label = isThought ? t.thoughtProcess : t.tools[step.tool];
  return (
    <div className="relative pl-12">
      <TreeGutter isLast={isLast} />
      {!isLast && <span className="absolute top-8 bottom-0 left-[12px] w-px bg-[#ffffff1f]" />}
      <span className="absolute top-2 left-8" style={{ color: muted }}>
        <Icon name={icon} size={16} />
      </span>
      <button
        type="button"
        className="group/step my-px ml-2 flex h-[30px] w-[calc(100%-8px)] cursor-pointer items-center gap-2 text-left text-[12px] leading-[18px]"
        style={{ color: muted }}
        aria-expanded={expanded}
        onClick={onToggle}
      >
        <span className="shrink-0 group-hover/step:text-[#f0eef4]">{label}</span>
        {!isThought && <span className="min-w-0 truncate group-hover/step:text-[#f0eef4]">{step.detail}</span>}
        {!isThought && (step.added || step.removed) && (
          <span className="flex shrink-0 gap-1 font-wu-mono text-[11px]">
            {step.added ? <span className="text-[#34d399]">+{step.added}</span> : null}
            {step.removed ? <span className="text-[#f87171]">−{step.removed}</span> : null}
          </span>
        )}
        <span className="grid size-[18px] shrink-0 place-items-center opacity-0 group-hover/step:opacity-100">
          <Icon name={expanded ? 'agent_arrow_down' : 'agent_arrow_right'} size={12} color={placeholder} />
        </span>
      </button>
      {expanded && (
        <div className="ml-2 py-1.5 pb-2 text-[12px] leading-[18px]" style={{ color: placeholder }}>
          {isThought ? (
            <p className="text-[15px] leading-[18px]">
              <InlineMarkdown source={step.text} codeClassName="font-wu-mono" />
            </p>
          ) : (
            <p className="font-wu-mono">{step.tool === 'run' ? 'test result: ok. 214 passed; 0 failed' : step.detail}</p>
          )}
        </div>
      )}
    </div>
  );
}

export function stepsSummary(steps: ChatStep[], t: Messages['wuWindow']['summary']) {
  const thoughts = steps.filter((step) => step.kind === 'thought').length;
  const tools = steps.flatMap((step) => (step.kind === 'tool' ? [step] : []));
  const count = (tool: ToolKind) => tools.filter((step) => step.tool === tool).length;
  const editedFiles = new Set(tools.filter((step) => step.tool === 'edit').map((step) => step.detail)).size;
  const toolSegments = [
    count('run') && t.ran(count('run')),
    editedFiles && t.edited(editedFiles),
    count('read') && t.read(count('read')),
    count('search') && t.searched(count('search'))
  ].filter((segment): segment is string => Boolean(segment));
  const toolSummary = toolSegments.join(t.separator);
  const segments = [
    thoughts === 1 ? t.thoughtOnce : thoughts > 1 ? t.thoughtTimes(thoughts) : '',
    toolSummary && toolSummary[0]?.toUpperCase() + toolSummary.slice(1)
  ].filter(Boolean);
  const summary = segments.join(t.separator);
  return summary.charAt(0).toUpperCase() + summary.slice(1);
}

function HoverStrip({
  time,
  alignEnd,
  saved,
  onToggleSaved
}: {
  time?: string;
  alignEnd: boolean;
  saved: boolean;
  onToggleSaved: () => void;
}) {
  const t = useMessages().wuWindow;
  return (
    <div
      className={`flex h-8 items-center gap-2 pt-2 opacity-0 transition-opacity group-hover/turn:opacity-100 ${alignEnd ? 'justify-end' : ''}`}
    >
      {time && <span className="text-[12px] text-[#aaa8b78c]">{time}</span>}
      <span className="grid size-6 place-items-center rounded-[6px] hover:bg-[#ffffff14]">
        <Icon name="agent_copy" size={14} color={muted} />
      </span>
      <button
        type="button"
        title={saved ? t.agentPanel.removeSaved : t.chat.saveMessage}
        aria-pressed={saved}
        className="grid size-6 cursor-pointer place-items-center rounded-[6px] hover:bg-[#ffffff14]"
        onClick={onToggleSaved}
      >
        <Icon name={saved ? 'agent_bookmark_filled' : 'agent_bookmark'} size={14} color={saved ? accent : muted} />
      </button>
    </div>
  );
}

export function UsageRing({ percent, color }: { percent: number; color: string }) {
  const radius = 6.1;
  const circumference = 2 * Math.PI * radius;
  return (
    <span className="flex h-6 items-center gap-[5px] rounded-[6px] px-1.5 text-[11px] hover:bg-[#ffffff0d]" style={{ color: muted }}>
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" className="-rotate-90">
        <circle cx="8" cy="8" r={radius} fill="none" stroke="#72708040" strokeWidth="1.8" />
        <circle
          cx="8"
          cy="8"
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth="1.8"
          strokeLinecap="round"
          strokeDasharray={`${(circumference * percent) / 100} ${circumference}`}
        />
      </svg>
      {percent}%
    </span>
  );
}

function ChatView({
  entries,
  draft,
  streaming,
  expandedSteps,
  savedEntries,
  onDraft,
  onSend,
  onToggleStep,
  onToggleSaved
}: {
  entries: ChatEntry[];
  draft: string;
  streaming: boolean;
  expandedSteps: Set<string>;
  onDraft: (text: string) => void;
  onSend: () => void;
  onToggleStep: (key: string) => void;
  savedEntries: Set<number>;
  onToggleSaved: (index: number) => void;
}) {
  const messages = useMessages();
  const t = messages.wuWindow.chat;
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastEntry = entries[entries.length - 1];
  const lastText = lastEntry?.kind === 'assistant' ? lastEntry.text : '';
  const canSend = draft.trim() !== '' && !streaming;

  useEffect(() => {
    const scroller = scrollRef.current;
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }, [entries.length, lastText]);

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-[#181a21]">
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-auto overscroll-contain">
        <div className="mx-auto flex max-w-[736px] flex-col px-12 pt-[26px] pb-8">
          {entries.map((entry, index) => {
            switch (entry.kind) {
              case 'user':
                return (
                  <div key={index} className={`group/turn flex flex-col items-end ${index > 0 ? 'pt-4' : ''}`}>
                    <div className="max-w-[80%] rounded-[16px] bg-[#ebebeb14] px-4 py-2.5 text-[14px] leading-[22px]" style={{ color: text }}>
                      {entry.text}
                    </div>
                    <HoverStrip time={entry.time} alignEnd saved={savedEntries.has(index)} onToggleSaved={() => onToggleSaved(index)} />
                  </div>
                );
              case 'steps': {
                const groupKey = `${index}`;
                const open = expandedSteps.has(groupKey);
                return (
                  <div key={index} className="pt-3">
                    <button
                      type="button"
                      className="flex h-[26px] cursor-pointer items-center gap-1.5 text-[12px] leading-[18px] hover:text-[#f0eef4]"
                      style={{ color: muted }}
                      aria-expanded={open}
                      onClick={() => onToggleStep(groupKey)}
                    >
                      <span className="relative h-[18px] w-[22px]">
                        <span className={`absolute top-0.5 left-[5.5px] transition-transform ${open ? '' : '-rotate-90'}`}>
                          <Icon name="agent_arrow_down" size={14} />
                        </span>
                      </span>
                      {stepsSummary(entry.steps, messages.wuWindow.summary)}
                    </button>
                    {open &&
                      entry.steps.map((step, stepIndex) => {
                        const stepKey = `${index}:${stepIndex}`;
                        return (
                          <StepRow
                            key={stepKey}
                            step={step}
                            isLast={stepIndex === entry.steps.length - 1}
                            expanded={expandedSteps.has(stepKey)}
                            onToggle={() => onToggleStep(stepKey)}
                          />
                        );
                      })}
                  </div>
                );
              }
              case 'assistant':
                return (
                  <div key={index} className="group/turn pt-3">
                    {entry.text ? (
                      <AssistantText source={entry.text} />
                    ) : (
                      <span className="text-[12px]" style={{ color: muted }}>
                        {t.thinking}
                      </span>
                    )}
                    {!(streaming && index === entries.length - 1) && (
                      <HoverStrip alignEnd={false} saved={savedEntries.has(index)} onToggleSaved={() => onToggleSaved(index)} />
                    )}
                  </div>
                );
            }
          })}
        </div>
      </div>
      <div className="mx-auto w-full max-w-[768px] px-4 pb-2">
        <div className="h-6" />
        <div className="flex h-[47px] items-center rounded-[22px] border border-[#2c2f3a] bg-[#2a2d37] pr-2 pl-2 shadow-[0_10px_15px_-3px_rgba(0,0,0,0.1),0_4px_6px_-4px_rgba(0,0,0,0.1)]">
          <span className={`grid size-7 shrink-0 place-items-center rounded-full ${hover}`}>
            <Icon name="agent_paperclip" size={18} color={muted} />
          </span>
          <input
            className="min-w-0 flex-1 bg-transparent px-2 text-[14px] outline-none placeholder:text-[#727080]"
            style={{ color: text }}
            placeholder={t.placeholder}
            aria-label={t.messageLabel}
            value={draft}
            onChange={(event) => onDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                onSend();
              }
            }}
          />
          <span className="flex h-8 shrink-0 items-center gap-1.5 rounded-[8px] px-1.5 text-[12px] font-medium text-[#f0eef4e6] hover:bg-[#ffffff1c]">
            <Icon name="agent_claude" size={16} color={claude} />
            Opus 5.5
            <span className="text-[#aaa8b7b3]">{t.effort}</span>
          </span>
          <span className="flex shrink-0 items-center gap-2 pl-0.5">
            <span className={`grid size-7 place-items-center rounded-full ${hover}`}>
              <Icon name="agent_microphone" size={18} color={muted} />
            </span>
            <button
              type="button"
              aria-label={t.send}
              disabled={!canSend}
              className={`grid size-7 place-items-center rounded-full bg-[#f0eef4] ${canSend ? 'cursor-pointer hover:opacity-85' : 'opacity-35'}`}
              onClick={onSend}
            >
              <Icon name="agent_send" size={14} color="#181a21" />
            </button>
          </span>
        </div>
        <div className="flex h-6 items-center pt-2 text-[12px] font-medium text-[#aaa8b799]">
          <span className="flex items-center gap-1 pl-2.5">
            <span className="flex h-5 items-center gap-1.5 px-2">
              <Icon name="agent_folder" size={12} />
              {t.checkout}
            </span>
            <span className="flex h-5 items-center gap-1.5 px-2">
              <Icon name="agent_git_branch" size={12} />
              main
            </span>
          </span>
          <span className="ml-auto flex items-center gap-1 pr-2.5 pl-1 font-normal">
            <UsageRing percent={18} color={accent} />
            <UsageRing percent={34} color={muted} />
          </span>
        </div>
      </div>
    </div>
  );
}

interface DiffSide {
  number?: number;
  code: string;
  changed: boolean;
  highlight?: [number, number];
}

interface DiffRow {
  left?: DiffSide;
  right?: DiffSide;
  inHunk: boolean;
}

function changedRange(before: string, after: string): [[number, number], [number, number]] {
  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) prefix += 1;
  let suffix = 0;
  while (
    suffix < before.length - prefix &&
    suffix < after.length - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  )
    suffix += 1;
  return [
    [prefix, before.length - suffix],
    [prefix, after.length - suffix]
  ];
}

function diffRows(file: MockFile): DiffRow[] {
  const lines = file.code.split('\n');
  const hunks = [...(file.hunks ?? [])].sort((first, second) => first.start - second.start);
  const rows: DiffRow[] = [];
  let oldNumber = file.firstLine;
  let index = 0;
  while (index < lines.length) {
    const hunk = hunks.find((candidate) => candidate.start === index);
    if (!hunk) {
      const code = lines[index] ?? '';
      rows.push({
        left: { number: oldNumber, code, changed: false },
        right: { number: file.firstLine + index, code, changed: false },
        inHunk: false
      });
      oldNumber += 1;
      index += 1;
      continue;
    }
    const before = hunk.before ?? [];
    const after = lines.slice(hunk.start, hunk.end + 1);
    for (let offset = 0; offset < Math.max(before.length, after.length); offset += 1) {
      const removed = before[offset];
      const added = after[offset];
      const ranges = removed !== undefined && added !== undefined ? changedRange(removed, added) : undefined;
      rows.push({
        left: removed === undefined ? undefined : { number: oldNumber + offset, code: removed, changed: true, highlight: ranges?.[0] },
        right: added === undefined ? undefined : { number: file.firstLine + hunk.start + offset, code: added, changed: true, highlight: ranges?.[1] },
        inHunk: true
      });
    }
    oldNumber += before.length;
    index = hunk.end + 1;
  }
  return rows;
}

function excerpts(rows: DiffRow[], context = 2) {
  const visible = rows.map((_, index) =>
    rows.slice(Math.max(0, index - context), index + context + 1).some((row) => row.inHunk)
  );
  const groups: DiffRow[][] = [];
  rows.forEach((row, index) => {
    if (!visible[index]) return;
    if (index > 0 && visible[index - 1]) groups[groups.length - 1]?.push(row);
    else groups.push([row]);
  });
  return groups;
}

function diffStats(file: MockFile) {
  const rows = diffRows(file);
  return {
    added: rows.filter((row) => row.right?.changed).length,
    removed: rows.filter((row) => row.left?.changed).length
  };
}

function DiffStat({ added, removed }: { added: number; removed: number }) {
  return (
    <span className="flex shrink-0 items-center gap-1 text-[12px]">
      <span className="text-[#34d399]">+{' '}{added}</span>
      <span className="text-[#f87171]">{'‒ '}{removed}</span>
    </span>
  );
}

function DiffCell({ side, kind, language }: { side?: DiffSide; kind: 'deleted' | 'added'; language: MockFile['language'] }) {
  const color = kind === 'deleted' ? '#f87171' : '#34d399';
  const background = side?.changed ? (kind === 'deleted' ? 'rgba(248,113,113,0.12)' : 'rgba(52,211,153,0.12)') : undefined;
  const wordBackground = kind === 'deleted' ? '#f871714c' : '#34d39940';
  const highlight = side?.highlight;
  return (
    <div className="relative flex min-w-0 flex-1 overflow-hidden" style={{ height: LINE_HEIGHT, lineHeight: `${LINE_HEIGHT}px`, background }}>
      {side?.changed && <span className="absolute inset-y-0 left-0 w-[6px]" style={{ background: color }} />}
      <span className="shrink-0 pr-[27px] pl-9 text-right select-none" style={{ width: 99 + 4.5, color: side?.changed ? color : '#555766' }}>
        {side?.number}
      </span>
      {side && (
        <span className="relative whitespace-pre" style={{ color: text }}>
          {highlight && highlight[1] > highlight[0] && (
            <span
              aria-hidden="true"
              className="absolute inset-y-0"
              style={{ left: `${highlight[0]}ch`, width: `${highlight[1] - highlight[0]}ch`, background: wordBackground }}
            />
          )}
          <span className="relative">
            <Highlighted code={side.code} language={language} />
          </span>
        </span>
      )}
    </div>
  );
}

function DiffView({ target }: { target: string }) {
  const t = useMessages().wuWindow.diff;
  const scrollRef = useRef<HTMLDivElement>(null);
  const headerRefs = useRef(new Map<string, HTMLDivElement>());
  const totals = changedFiles.map(diffStats).reduce(
    (sum, stats) => ({ added: sum.added + stats.added, removed: sum.removed + stats.removed }),
    { added: 0, removed: 0 }
  );

  useEffect(() => {
    const scroller = scrollRef.current;
    const header = headerRefs.current.get(target);
    if (scroller && header) scroller.scrollTop = Math.max(0, header.offsetTop - LINE_HEIGHT);
  }, [target]);

  const toolbarButton = (label: string, enabled = true) => (
    <span key={label} className={`flex h-[22px] items-center rounded-[4px] px-1 ${enabled ? hover : ''}`} style={{ color: enabled ? text : placeholder }}>
      {label}
    </span>
  );
  const divider = <span className="mx-0.5 h-4 w-px bg-[#23262f]" />;

  return (
    <>
      <div className="flex h-[45px] shrink-0 items-center gap-2 border-b border-[#23262f] bg-[#181a21] px-2 py-1.5 text-[14px]">
        <span className="flex gap-1 pl-0.5">
          <IconButton name="chevron_down_up" label={t.collapseAll} />
          <IconButton name="diff_unified" label={t.unified} />
          <IconButton name="diff_split" color={accent} label={t.split} />
        </span>
        <span className="ml-auto flex items-center gap-1.5">
          <DiffStat {...totals} />
          {divider}
          <span className="flex gap-0.5">
            <IconButton name="arrow_up" label={t.previousHunk} />
            <IconButton name="arrow_down" label={t.nextHunk} />
          </span>
          {divider}
          {toolbarButton(t.stage)}
          {toolbarButton(t.unstage, false)}
          {divider}
          <span className="flex w-20 justify-center">{toolbarButton(t.stageAll)}</span>
          {divider}
          {toolbarButton(t.commit)}
        </span>
      </div>
      <div ref={scrollRef} className="relative min-h-0 flex-1 overflow-auto overscroll-contain bg-[#181a21] font-wu-mono text-[15px]">
        <span aria-hidden="true" className="pointer-events-none absolute inset-y-0 left-1/2 w-px bg-[#23262f]" />
        {changedFiles.map((file) => {
          const isTarget = file.path === target;
          const stats = diffStats(file);
          return (
            <div key={file.path}>
              <div
                ref={(element) => {
                  if (element) headerRefs.current.set(file.path, element);
                }}
                className="group/header relative h-12 bg-[#181a21] p-[4.22px]"
              >
                <div className={`flex h-full items-center gap-[6.33px] rounded-[4.22px] border border-[#2c2f3a] pr-[8.44px] pl-[4.22px] ${hover}`}>
                  <span className="grid size-[29.5px] place-items-center rounded-[2px] hover:bg-[#7aa2f72e]">
                    <Icon name="chevron_down" size={17} color={text} />
                  </span>
                  <span className="grid size-5 place-items-center">
                    <span className="size-[17px] rounded-[2px] border border-[#2c2f3a] bg-[#181a21]" />
                  </span>
                  <span className="w-[12.66px]" />
                  <span className="flex min-w-0 items-center gap-[2.1px] text-[14.8px]">
                    <ImageIcon src={fileIcon(file.path)} size={17} />
                    <span className="pl-1 text-[#facc15]">{fileName(file.path)}</span>
                    <span className="truncate pl-2" style={{ color: muted }}>
                      {directory(file.path) && `${directory(file.path)}/`}
                    </span>
                  </span>
                  <span className="ml-auto flex items-center gap-[8.44px] font-wu">
                    <DiffStat {...stats} />
                    <span
                      className={`flex h-[22px] items-center gap-1.5 rounded-[4px] border border-[#23262f] px-1.5 text-[14px] ${isTarget ? '' : 'invisible group-hover/header:visible'}`}
                    >
                      {t.openFile}
                      <span className="text-[12px]" style={{ color: muted }}>
                        ⌥↩
                      </span>
                    </span>
                  </span>
                </div>
              </div>
              {excerpts(diffRows(file)).map((group, groupIndex) => (
                <div key={groupIndex}>
                  {groupIndex > 0 && (
                    <div className="relative h-6">
                      <span className="absolute inset-x-0 top-3 h-px bg-[#23262f]" />
                    </div>
                  )}
                  {group.map((row, rowIndex) => (
                    <div key={rowIndex} className="flex">
                      <DiffCell side={row.left} kind="deleted" language={file.language} />
                      <span className="w-px shrink-0" />
                      <DiffCell side={row.right} kind="added" language={file.language} />
                    </div>
                  ))}
                </div>
              ))}
            </div>
          );
        })}
        <div style={{ height: LINE_HEIGHT * 10 }} />
      </div>
    </>
  );
}

const SAVED_PREVIEW_CHARS = 240;

function savedPreview(source: string) {
  let preview = '';
  for (const word of source.split(/\s+/).filter(Boolean)) {
    if (preview.length + word.length > SAVED_PREVIEW_CHARS) return `${preview}…`;
    preview = preview ? `${preview} ${word}` : word;
  }
  return preview;
}

const directory = (path: string) => path.slice(0, Math.max(0, path.length - fileName(path).length - 1));

export function WuWindow() {
  const t = useMessages().wuWindow;
  const { chats, previewReply } = t;
  const [panel, setPanel] = useState<PanelKey | null>('explorer');
  const [expanded, setExpanded] = useState(() => new Set(initiallyExpanded));
  const [tabs, setTabs] = useState<TabId[]>([
    'file:crates/workspace/src/pane.rs',
    'file:crates/agent_chat/src/session.rs',
    'chat:preview-tests'
  ]);
  const [activeTab, setActiveTab] = useState<TabId | null>('file:crates/workspace/src/pane.rs');
  const [cursors, setCursors] = useState<Record<string, { line: number; column: number }>>({
    'crates/workspace/src/pane.rs': { line: 12, column: 64 }
  });
  const [revealToken, setRevealToken] = useState(0);
  const [query, setQuery] = useState('preview_item');
  const [chatEntries, setChatEntries] = useState(() => Object.fromEntries(chats.map((chat) => [chat.id, chat.entries])));
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [streamingChat, setStreamingChat] = useState<string | null>(null);
  const [expandedSteps, setExpandedSteps] = useState<Record<string, Set<string>>>({});
  const [savedMessages, setSavedMessages] = useState<{ chatId: string; index: number }[]>([]);
  const [showingSaved, setShowingSaved] = useState(false);
  const streamTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(
    () => () => {
      if (streamTimer.current) clearInterval(streamTimer.current);
    },
    []
  );

  const activeFile = activeTab?.startsWith('file:') ? fileByPath.get(activeTab.slice(5)) : undefined;
  const [diffTarget, setDiffTarget] = useState(changedFiles[0]?.path ?? '');
  const activeChat = activeTab?.startsWith('chat:') ? chats.find((chat) => chat.id === activeTab.slice(5)) : undefined;
  const cursor = activeFile ? (cursors[activeFile.path] ?? { line: 0, column: 1 }) : undefined;

  const openTab = (tab: TabId) => {
    setTabs((current) => {
      if (current.includes(tab)) return current;
      const insertAt = activeTab ? current.indexOf(activeTab) + 1 : current.length;
      return [...current.slice(0, insertAt), tab, ...current.slice(insertAt)];
    });
    setActiveTab(tab);
  };

  const openFile = (path: string, line?: number, column?: number) => {
    openTab(`file:${path}`);
    if (line !== undefined) {
      setCursors((current) => ({ ...current, [path]: { line, column: column ?? 1 } }));
      setRevealToken((token) => token + 1);
    }
  };

  const closeTab = (tab: TabId) => {
    const index = tabs.indexOf(tab);
    const remaining = tabs.filter((existing) => existing !== tab);
    setTabs(remaining);
    if (activeTab === tab) setActiveTab(remaining[Math.min(index, remaining.length - 1)] ?? null);
  };

  const sendMessage = (chatId: string) => {
    const message = drafts[chatId]?.trim();
    if (!message || streamingChat) return;
    setDrafts((current) => ({ ...current, [chatId]: '' }));
    setChatEntries((current) => ({
      ...current,
      [chatId]: [...(current[chatId] ?? []), { kind: 'user', text: message, time: t.chat.justNow }, { kind: 'assistant', text: '' }]
    }));
    setStreamingChat(chatId);
    const words = previewReply.split(' ');
    let shown = -8;
    streamTimer.current = setInterval(() => {
      shown += 1;
      if (shown <= 0) return;
      setChatEntries((current) => {
        const entries = [...(current[chatId] ?? [])];
        entries[entries.length - 1] = { kind: 'assistant', text: words.slice(0, shown).join(' ') };
        return { ...current, [chatId]: entries };
      });
      if (shown >= words.length) {
        if (streamTimer.current) clearInterval(streamTimer.current);
        streamTimer.current = null;
        setStreamingChat(null);
      }
    }, 45);
  };

  const toggleSaved = (chatId: string, index: number) =>
    setSavedMessages((current) =>
      current.some((saved) => saved.chatId === chatId && saved.index === index)
        ? current.filter((saved) => saved.chatId !== chatId || saved.index !== index)
        : [{ chatId, index }, ...current]
    );

  const renderSavedMessages = () => {
    const rows = savedMessages.flatMap(({ chatId, index }) => {
      const chat = chats.find((candidate) => candidate.id === chatId);
      const entry = chatEntries[chatId]?.[index];
      if (!chat || (entry?.kind !== 'user' && entry?.kind !== 'assistant')) return [];
      return [{ chat, index, fromAgent: entry.kind === 'assistant', text: savedPreview(entry.text) }];
    });
    if (rows.length === 0) {
      return (
        <div className="px-4 pt-3 text-[12px]" style={{ color: placeholder }}>
          {t.agentPanel.noSaved}
        </div>
      );
    }
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-auto px-2 pt-1 pb-2">
        {rows.map(({ chat, index, fromAgent, text: preview }) => (
          <div
            key={`${chat.id}:${index}`}
            className="group/saved flex cursor-pointer flex-col gap-1 rounded-[8px] p-2 text-[#f0eef4cc] hover:bg-[#ffffff1c] hover:text-[#f0eef4]"
            onClick={() => openTab(`chat:${chat.id}`)}
          >
            <div className="flex items-start gap-2">
              <span className="line-clamp-3 min-w-0 flex-1 text-[13px] leading-[18px]">{preview}</span>
              <button
                type="button"
                title={t.agentPanel.removeSaved}
                className="invisible grid size-[18px] shrink-0 cursor-pointer place-items-center rounded-[5px] group-hover/saved:visible hover:bg-[#ffffff1c]"
                onClick={(event) => {
                  event.stopPropagation();
                  toggleSaved(chat.id, index);
                }}
              >
                <Icon name="agent_close" size={11} color={muted} />
              </button>
            </div>
            <div className="flex min-w-0 items-center gap-1 text-[11px] leading-[14px] text-[#aaa8b7b3]">
              <Icon name={fromAgent ? 'agent_bot' : 'agent_chat_round_line'} size={11} />
              <span className="truncate">
                {fromAgent ? 'Claude' : t.agentPanel.you} · {chat.title} · {t.agentPanel.now}
              </span>
            </div>
          </div>
        ))}
      </div>
    );
  };

  const toggleFolder = (path: string) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });

  const indentGuides = (depth: number) =>
    Array.from({ length: depth }, (_, level) => (
      <span key={level} aria-hidden="true" className="absolute inset-y-0 w-px bg-[#fefef31b]" style={{ left: level * 20 + 15 }} />
    ));

  const renderTree = (node: TreeNode, depth: number): ReactNode => {
    const rowClass = 'relative flex h-6 w-full cursor-pointer items-center gap-1.5 border border-transparent pr-[2px] text-left text-[14px]';
    if (typeof node === 'string') {
      const file = fileByPath.get(node);
      const isMarked = activeFile?.path === node;
      const color = file?.modified ? '#facc15' : isMarked ? text : muted;
      return (
        <button
          key={node}
          type="button"
          className={`${rowClass} ${isMarked ? 'bg-[#7aa2f72e]' : hover}`}
          style={{ paddingLeft: 6 + depth * 20, color }}
          onClick={() => openFile(node)}
        >
          {indentGuides(depth)}
          <ImageIcon src={fileIcon(node)} />
          <span className="truncate">{fileName(node)}</span>
        </button>
      );
    }
    const isOpen = expanded.has(node.path);
    return (
      <div key={node.path || 'root'}>
        <button
          type="button"
          className={`${rowClass} ${hover}`}
          style={{ paddingLeft: 6 + depth * 20, color: muted }}
          aria-expanded={isOpen}
          onClick={() => toggleFolder(node.path)}
        >
          {indentGuides(depth)}
          <ImageIcon src={folderIcon(node.name)} />
          <span className="truncate">{node.name}</span>
        </button>
        {isOpen && node.children.map((child) => renderTree(child, depth + 1))}
      </div>
    );
  };

  const searchResults = (() => {
    const needle = query.trim().toLowerCase();
    if (needle.length < 2) return [];
    return files
      .map((file) => ({
        file,
        matches: file.code
          .split('\n')
          .map((line, index) => ({ line, index, column: line.toLowerCase().indexOf(needle) }))
          .filter((match) => match.column >= 0)
      }))
      .filter((result) => result.matches.length > 0);
  })();

  const renderSidebar = () => {
    switch (panel) {
      case 'explorer':
        return <div className="min-h-0 flex-1 overflow-auto overscroll-contain pb-8">{renderTree(tree, 0)}</div>;
      case 'search': {
        const needle = query.trim();
        const matchCount = searchResults.reduce((count, result) => count + result.matches.length, 0);
        return (
          <>
            <div className="flex gap-1 px-2 pt-2 pb-1">
              <span className={`grid w-4 shrink-0 place-items-center rounded-[4px] ${hover}`}>
                <Icon name="chevron_right" size={14} color={muted} />
              </span>
              <div className="flex h-7 min-w-0 flex-1 items-center gap-1 rounded-[6px] border border-[#23262f] bg-[#181a21] pr-0.5 pl-2 focus-within:border-[#7aa2f7b2]">
                <input
                  className="min-w-0 flex-1 bg-transparent text-[14px] outline-none placeholder:text-[#727080]"
                  style={{ color: text }}
                  placeholder={t.search.placeholder}
                  aria-label={t.search.label}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                />
                <IconButton name="case_sensitive" size={16} box={20} label={t.search.matchCase} />
                <IconButton name="whole_word" size={16} box={20} label={t.search.wholeWords} />
                <IconButton name="regex" size={16} box={20} label={t.search.regex} />
              </div>
            </div>
            <div className="flex h-7 shrink-0 items-center gap-2 border-b border-[#23262f] pr-1 pl-2">
              <span className="flex-1 truncate text-[12px]" style={{ color: muted }}>
                {needle.length >= 2 && t.search.results(matchCount, searchResults.length)}
              </span>
              {['arrow_circle', 'eraser', 'list_collapse', 'file_diff', 'ellipsis'].map((name) => (
                <IconButton key={name} name={name} box={18} label={name} />
              ))}
            </div>
            <div className="min-h-0 flex-1 overflow-auto overscroll-contain pt-0.5">
              {searchResults.map(({ file, matches }) => (
                <div key={file.path}>
                  <div className="flex h-[23px] items-center gap-1.5 border-l-2 border-transparent pr-2 pl-1.5">
                    <ImageIcon src={fileIcon(file.path)} size={14} />
                    <span className="shrink-0 text-[14px]" style={{ color: text }}>
                      {fileName(file.path)}
                    </span>
                    <span className="min-w-0 truncate text-[12px]" style={{ color: muted }}>
                      {directory(file.path)}
                    </span>
                    <span className="ml-auto rounded-full bg-[#2a2d37] px-1.5 text-[10px]" style={{ color: muted }}>
                      {matches.length}
                    </span>
                  </div>
                  {matches.map((match) => {
                    const leading = match.line.length - match.line.trimStart().length;
                    const start = match.column - leading;
                    const trimmed = match.line.trimStart();
                    return (
                      <button
                        key={match.index}
                        type="button"
                        className={`flex h-[23px] w-full cursor-pointer items-center border-l-2 border-transparent pr-2 pl-7 text-left text-[12px] ${hover}`}
                        style={{ color: muted }}
                        onClick={() => openFile(file.path, match.index, match.column + 1)}
                      >
                        <span className="truncate whitespace-pre">
                          {trimmed.slice(0, start)}
                          <span className="bg-[#7aa2f74c]" style={{ color: text }}>
                            {trimmed.slice(start, start + needle.length)}
                          </span>
                          {trimmed.slice(start + needle.length)}
                        </span>
                      </button>
                    );
                  })}
                </div>
              ))}
            </div>
          </>
        );
      }
      case 'git':
        return (
          <>
            <div className="flex h-8 shrink-0 text-[14px]">
              <span className="flex flex-1 items-center justify-center gap-1" style={{ color: text }}>
                {t.git.changes}
                <span className="text-[12px]" style={{ color: muted }}>
                  ({changedFiles.length})
                </span>
              </span>
              <span className="w-px bg-[#2c2f3a]" />
              <span className="flex flex-1 items-center justify-center border-b border-[#2c2f3a99] bg-[#181a2199]" style={{ color: muted }}>
                {t.git.history}
              </span>
            </div>
            <div className="flex min-h-8 shrink-0 items-center pr-2 pl-1">
              <span className={`flex h-[22px] items-center gap-1 rounded-[4px] px-1 text-[12px] ${hover}`}>
                <Icon name="diff" size={14} color={muted} />
                {t.git.viewDiff}
                <span className="pl-1 font-wu-mono text-[11px]">
                  <span className="text-[#34d399]">+75</span> <span className="text-[#f87171]">−8</span>
                </span>
              </span>
              <span className="ml-auto flex gap-0.5">
                <IconButton name="ellipsis" label={t.git.viewOptions} />
              </span>
            </div>
            <div className="flex h-7 shrink-0 items-center gap-2 pr-1 pl-2.5 text-[12px]" style={{ color: muted }}>
              <Icon name="chevron_down" size={12} />
              {t.git.tracked}
            </div>
            <div className="min-h-0 flex-1 overflow-auto">
              {changedFiles.map((file) => (
                <button
                  key={file.path}
                  type="button"
                  className={`flex h-7 w-full cursor-pointer items-center gap-1.5 border border-transparent pr-1 pl-2.5 text-left text-[14px] ${activeTab === 'diff' && diffTarget === file.path ? 'bg-[#60a5fa14]' : hover}`}
                  onClick={() => {
                    setDiffTarget(file.path);
                    openTab('diff');
                  }}
                >
                  <Icon name="square_dot" size={16} color="#facc15" />
                  <span className="shrink-0" style={{ color: text }}>
                    {fileName(file.path)}
                  </span>
                  <span className="min-w-0 truncate" style={{ color: muted }}>
                    {directory(file.path)}
                  </span>
                </button>
              ))}
            </div>
            <div className="flex items-center gap-1 px-2 py-1.5">
              <Icon name="git_branch" size={14} color={muted} />
              <span className={`rounded-[4px] px-1 text-[12px] ${hover}`}>main</span>
            </div>
            <div className="h-[142px] border-t border-[#2c2f3a] bg-[#181a21] px-2 pt-2 font-wu-mono text-[12px] leading-[19px]" style={{ color: placeholder }}>
              {t.git.commitMessage}
            </div>
            <div className="flex justify-end p-1.5">
              <span className="flex h-[22px] items-center overflow-hidden rounded-[4px] bg-[#2a2d37] text-[12px]" style={{ color: text }}>
                <span className="px-2">{t.git.commitTracked}</span>
                <span className="grid h-full w-5 place-items-center border-l border-[#2c2f3a]">
                  <Icon name="chevron_down" size={12} color={muted} />
                </span>
              </span>
            </div>
          </>
        );
      case 'outline': {
        const items = activeFile ? outline(activeFile) : [];
        return (
          <>
            <div className="flex h-8 shrink-0 items-center gap-2 border-b border-[#2c2f3a] px-2">
              <Icon name="magnifying_glass" size={14} color={muted} />
              <span className="flex-1 text-[14px]" style={{ color: placeholder }}>
                {t.outline.search}
              </span>
              <IconButton name="pin" size={16} box={20} color={muted} label={t.outline.pin} />
            </div>
            <div className="min-h-0 flex-1 overflow-auto overscroll-contain">
              {activeFile &&
                items.map((item, index) => {
                  const hasChildren = (items[index + 1]?.depth ?? 0) > item.depth;
                  const isActive = cursor ? symbolPath(activeFile, cursor.line).at(-1) === item.label : false;
                  return (
                    <button
                      key={`${item.line}:${item.label}`}
                      type="button"
                      className={`flex h-[26px] w-full cursor-pointer items-center gap-1 border border-transparent pr-[2px] text-left ${isActive ? 'bg-[#7aa2f72e]' : hover}`}
                      style={{ paddingLeft: 6 + item.depth * 20 }}
                      onClick={() => openFile(activeFile.path, item.line, 1)}
                    >
                      <span className="grid w-4 shrink-0 place-items-center">
                        {hasChildren && <Icon name="chevron_down" size={14} color={muted} />}
                      </span>
                      <span className="truncate font-wu-mono text-[15px] leading-none" style={{ color: text }}>
                        {activeFile.language === 'rust' ? <Highlighted code={item.label} language="rust" /> : item.label}
                      </span>
                    </button>
                  );
                })}
            </div>
          </>
        );
      }
      case 'debug':
        return (
          <>
            <div className="flex h-1/3 shrink-0 flex-col items-center justify-center gap-2 text-[14px]" style={{ color: text }}>
              {[
                ['plus', t.debug.newSession],
                ['code', t.debug.editConfig],
                ['book', t.debug.docs],
                ['blocks', t.debug.extensions]
              ].map(([icon = '', label]) => (
                <span key={label} className={`flex h-[22px] items-center gap-1 rounded-[4px] px-1 ${hover}`}>
                  <Icon name={icon} size={14} color={muted} />
                  {label}
                </span>
              ))}
            </div>
            <div className="flex items-center border-b border-[#23262f] p-1.5 text-[12px]" style={{ color: text }}>
              {t.debug.breakpoints}
            </div>
            <div className="p-2 text-[12px]" style={{ color: muted }}>
              {t.debug.noBreakpoints}
            </div>
          </>
        );
      case 'agent':
        return (
          <div className="flex min-h-0 flex-1 flex-col">
            <div className="flex items-center gap-1 px-2 pt-2 pb-1">
              <span className="flex h-[29px] flex-1 items-center px-2 text-[13px] font-medium text-[#f0eef4cc]">
                {showingSaved ? t.agentPanel.savedMessages : t.agentPanel.chats}
              </span>
              <button
                type="button"
                title={showingSaved ? t.agentPanel.backToChats : t.agentPanel.savedMessages}
                aria-pressed={showingSaved}
                className={`grid size-[29px] cursor-pointer place-items-center rounded-[8px] ${showingSaved ? 'bg-[#ffffff1c]' : hover}`}
                onClick={() => setShowingSaved((current) => !current)}
              >
                <Icon name="bookmark" size={16} color="#aaa8b7cc" />
              </button>
              <span className={`grid size-[29px] place-items-center rounded-[8px] ${hover}`}>
                <Icon name="plus" size={16} color="#aaa8b7cc" />
              </span>
            </div>
            <div className="px-2 pb-1">
              <div className="flex items-center gap-2 rounded-[7px] bg-[#ffffff0a] px-2.5 py-1.5 text-[13px]" style={{ color: placeholder }}>
                <Icon name="agent_magnifer" size={16} color={muted} />
                {showingSaved ? t.agentPanel.searchSaved : t.agentPanel.searchChats}
              </div>
            </div>
            {showingSaved ? (
              renderSavedMessages()
            ) : (
              <div className="min-h-0 flex-1 overflow-auto px-2 pt-1">
                <div className="pt-3">
                  <div className="flex h-7 items-center gap-2 px-2 text-[12px] font-medium text-[#aaa8b780]">
                    {t.agentPanel.chats}
                    <span className="ml-auto rotate-90">
                      <Icon name="agent_arrow_right" size={12} />
                    </span>
                  </div>
                  <div className="flex flex-col gap-0.5 pt-1">
                    {chats.map((chat) => {
                      const isSelected = activeChat?.id === chat.id;
                      return (
                        <button
                          key={chat.id}
                          type="button"
                          className={`flex h-[29px] w-full cursor-pointer items-center rounded-[8px] px-2 py-1.5 text-left ${isSelected ? 'bg-[#ebebeb1c] text-[#f0eef4]' : 'text-[#f0eef4cc] hover:bg-[#ffffff1c] hover:text-[#f0eef4]'}`}
                          onClick={() => openTab(`chat:${chat.id}`)}
                        >
                          <span className="flex min-w-0 flex-1 items-center gap-2 text-[13px] leading-[17px]">
                            <Icon name="agent_claude" size={13} color="#d97757cc" />
                            <span className="truncate">{chat.title}</span>
                            <span className="ml-auto shrink-0 text-[10px] font-medium text-[#aaa8b780]">{chat.age}</span>
                          </span>
                        </button>
                      );
                    })}
                  </div>
                </div>
              </div>
            )}
          </div>
        );
      case null:
        return null;
    }
  };

  const tabParts = (tab: TabId, isActive: boolean) => {
    if (tab === 'diff') return { title: t.tabs.uncommittedChanges, icon: <Icon name="git_branch" size={14} color={muted} /> };
    if (tab.startsWith('chat:')) {
      const chat = chats.find((candidate) => candidate.id === tab.slice(5));
      return { title: chat?.title ?? t.tabs.newChat, icon: <Icon name="agent_claude" size={14} color={claude} /> };
    }
    const path = tab.slice(5);
    return { title: fileName(path), icon: <ImageIcon src={fileIcon(path)} size={14} className={isActive ? '' : 'opacity-78'} /> };
  };

  const symbols = activeFile && cursor ? symbolPath(activeFile, cursor.line) : [];

  return (
    <div className="flex flex-col overflow-hidden bg-[#181a21] font-wu text-[14px] tracking-normal select-none" style={{ width: WIDTH, height: HEIGHT, color: text }}>
      <div className="relative flex h-[34px] shrink-0 items-center gap-0.5 pl-[71px] text-[12px]">
        <span className="absolute top-1/2 left-[13px] flex -translate-y-1/2 gap-2" aria-hidden="true">
          <span className="size-3 rounded-full bg-[#ff5f57]" />
          <span className="size-3 rounded-full bg-[#febc2e]" />
          <span className="size-3 rounded-full bg-[#28c840]" />
        </span>
        <span className={`flex h-[22px] items-center rounded-[4px] px-1 ${hover}`}>wu</span>
        <span className="flex items-center gap-px" style={{ color: muted }}>
          <span className={`flex h-[22px] items-center gap-1 rounded-[4px] px-1 ${hover}`}>
            <Icon name="git_worktree" size={12} />
            main
          </span>
          <span className="text-[#aaa8b740]">/</span>
          <span className={`flex h-[22px] items-center gap-1 rounded-[4px] px-1 ${hover}`}>
            <Icon name="git_branch" size={12} />
            main
          </span>
        </span>
      </div>

      <div className="flex min-h-0 flex-1 border-y border-[#2c2f3a]">
        <div className="flex w-12 shrink-0 flex-col items-center gap-2 border-r border-[#2c2f3a] py-1">
          {panels.map((item) => {
            const isActive = panel === item.key;
            return (
              <button
                key={item.key}
                type="button"
                aria-label={t.panels[item.key]}
                aria-pressed={isActive}
                title={t.panels[item.key]}
                className={`relative grid size-10 cursor-pointer place-items-center rounded-[4px] ${hover} active:bg-[#ffffff29]`}
                style={{ color: isActive ? accent : text }}
                onClick={() => setPanel(isActive ? null : item.key)}
              >
                <Icon name={item.icon} size={24} />
                {item.key === 'git' && (
                  <span className="absolute top-0 right-0 grid h-3.5 min-w-3.5 place-items-center rounded-full border border-[#2c2f3a] bg-[#693333] p-px text-[9px] leading-none font-medium shadow-sm" style={{ color: text }}>
                    {changedFiles.length}
                  </span>
                )}
              </button>
            );
          })}
          <span className={`mt-auto grid size-10 place-items-center rounded-[4px] ${hover}`} title={t.manage}>
            <Icon name="activity_settings" size={24} />
          </span>
        </div>

        {panel && (
          <div className="flex shrink-0 flex-col border-r border-[#2c2f3a]" style={{ width: PANEL_WIDTH }}>
            {renderSidebar()}
          </div>
        )}

        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-8 shrink-0 items-center">
            <div className="flex gap-1 p-1.5">
              <IconButton name="arrow_left" color={placeholder} label={t.tabs.goBack} />
              <IconButton name="arrow_right" color={placeholder} label={t.tabs.goForward} />
            </div>
            <div className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden px-1">
              {tabs.map((tab) => {
                const isActive = tab === activeTab;
                const { title, icon } = tabParts(tab, isActive);
                return (
                  <div
                    key={tab}
                    className={`group/tab flex h-6 max-w-[240px] shrink-0 cursor-pointer items-center gap-[3px] rounded-[6px] px-1 text-[14px] ${isActive ? 'bg-[#22252e]' : 'hover:bg-[#ebebeb10]'}`}
                    style={{ color: isActive ? text : muted }}
                    onClick={() => setActiveTab(tab)}
                  >
                    <span className="grid size-[18px] place-items-center">{icon}</span>
                    <span className="truncate">{title}</span>
                    <button
                      type="button"
                      aria-label={t.tabs.close(title)}
                      className={`invisible grid size-[18px] cursor-pointer place-items-center rounded-[4px] group-hover/tab:visible ${hover}`}
                      onClick={(event) => {
                        event.stopPropagation();
                        closeTab(tab);
                      }}
                    >
                      <Icon name="close" size={14} color={muted} />
                    </button>
                  </div>
                );
              })}
              <span className="grid h-8 place-items-center px-0.5">
                <IconButton name="plus" label={t.tabs.new} />
              </span>
            </div>
            <div className="flex gap-1 p-1.5">
              <IconButton name="split" label={t.tabs.splitPane} />
              <IconButton name="maximize" label={t.tabs.zoomIn} />
            </div>
          </div>

          {activeFile && cursor && (
            <>
              <div className="flex h-[45px] shrink-0 items-center border-b border-[#23262f] bg-[#181a21] px-2 py-1.5">
                <span className={`flex h-[22px] min-w-0 items-center gap-1 rounded-[4px] px-1 font-wu-mono text-[14px] ${hover}`} style={{ color: muted }}>
                  <span className="truncate">{activeFile.path}</span>
                  {symbols.map((symbol) => (
                    <span key={symbol} className="flex shrink-0 items-center gap-1">
                      <span className="font-wu" style={{ color: placeholder }}>
                        ›
                      </span>
                      {activeFile.language === 'rust' ? <Highlighted code={symbol} language="rust" /> : symbol}
                    </span>
                  ))}
                </span>
                <span className="ml-auto flex gap-0.5">
                  <IconButton name="activity_search" label={t.toolbar.bufferSearch} />
                  <IconButton name="cursor_i_beam" label={t.toolbar.selectionControls} />
                  <IconButton name="filter" label={t.toolbar.editorControls} />
                </span>
              </div>
              <Editor
                file={activeFile}
                cursor={cursor}
                revealToken={revealToken}
                onCursor={(line, column) => setCursors((current) => ({ ...current, [activeFile.path]: { line, column } }))}
              />
            </>
          )}

          {activeChat && (
            <ChatView
              entries={chatEntries[activeChat.id] ?? []}
              draft={drafts[activeChat.id] ?? ''}
              streaming={streamingChat === activeChat.id}
              expandedSteps={expandedSteps[activeChat.id] ?? new Set()}
              savedEntries={new Set(savedMessages.filter((saved) => saved.chatId === activeChat.id).map((saved) => saved.index))}
              onDraft={(value) => setDrafts((current) => ({ ...current, [activeChat.id]: value }))}
              onSend={() => sendMessage(activeChat.id)}
              onToggleStep={(key) =>
                setExpandedSteps((current) => {
                  const next = new Set(current[activeChat.id]);
                  if (next.has(key)) next.delete(key);
                  else next.add(key);
                  return { ...current, [activeChat.id]: next };
                })
              }
              onToggleSaved={(index) => toggleSaved(activeChat.id, index)}
            />
          )}

          {activeTab === 'diff' && <DiffView target={diffTarget} />}

          {!activeTab && <div className="flex-1 bg-[#181a21]" />}
        </div>
      </div>

      <div className="flex h-[30px] shrink-0 items-center justify-between gap-2 p-1 text-[12px]">
        <span className="flex items-center gap-1">
          {activeFile?.language === 'rust' && <IconButton name="bolt_outlined" label={t.statusBar.languageServers} />}
          <IconButton name="check" label={t.statusBar.diagnostics} />
        </span>
        <span className="flex items-center gap-1">
          {activeFile && cursor && (
            <>
              <span className={`flex h-[22px] items-center rounded-[4px] px-1 ${hover}`}>
                {activeFile.firstLine + cursor.line}:{cursor.column}
              </span>
              <span className={`flex h-[22px] items-center rounded-[4px] px-1 ${hover}`}>{languageName(activeFile.language)}</span>
            </>
          )}
          <span className="h-4 w-px bg-[#2c2f3a]" />
          <IconButton name="terminal_alt" label={t.statusBar.terminal} />
        </span>
      </div>
    </div>
  );
}

export function WuWindowFrame({ className }: { className: string }) {
  return (
    <div className={`@container relative aspect-[16/10] overflow-hidden ${className}`}>
      <div className="absolute top-0 left-0 origin-top-left" style={{ width: WIDTH, height: HEIGHT, scale: `tan(atan2(100cqw, ${WIDTH}px))` }}>
        <WuWindow />
      </div>
    </div>
  );
}
