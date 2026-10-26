import { useEffect, useRef, useState } from 'react';
import { useMessages } from '../i18n/locale';
import { mediaFrame } from './Section';
import { AssistantText, Icon, StepRow, UsageRing, accent, claude, muted, placeholder, stepsSummary, text } from './WuWindow';

const WIDTH = 760;
const HEIGHT = 475;
const TICK = 100;

const TYPE_START = 600;
const TYPE_SPEED = 32;
// Chinese messages are much shorter, so typing them at the English speed would look rushed.
const MIN_TYPING_TIME = 1600;

function timeline(message: string) {
  const typeSpeed = Math.max(TYPE_SPEED, MIN_TYPING_TIME / message.length);
  const sentAt = TYPE_START + message.length * typeSpeed + 500;
  const replyAt = sentAt + 7000;
  return {
    typeSpeed,
    sentAt,
    stepTimes: [900, 2100, 2900, 3900, 5100].map((delay) => sentAt + delay),
    replyAt,
    loopEnd: replyAt + 7000
  };
}

type Timeline = ReturnType<typeof timeline>;

const spinnerTints = ['#b6d3ef', '#edb185', '#f888a0'];

function WorkingSpinner() {
  return (
    <span className="flex flex-col gap-[1.25px]">
      {spinnerTints.map((tint, row) => (
        <span key={tint} className="flex gap-[1.25px]">
          {[0, 1, 2].map((column) => {
            const phase = (2 - row + Math.abs(column - 1)) / 4;
            return (
              <span
                key={column}
                className="size-[2.5px] animate-gspin rounded-full motion-reduce:animate-none"
                style={{ background: tint, animationDelay: `${(phase - 1) * 750}ms` }}
              />
            );
          })}
        </span>
      ))}
    </span>
  );
}

function useLoopTime(loopEnd: number) {
  const frameRef = useRef<HTMLDivElement>(null);
  const [time, setTime] = useState(loopEnd - TICK);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const frame = frameRef.current;
    if (!frame || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    setTime(0);
    const observer = new IntersectionObserver(([entry]) => setVisible(Boolean(entry?.isIntersecting)), { threshold: 0.4 });
    observer.observe(frame);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!visible) return;
    const interval = setInterval(() => setTime((current) => (current + TICK) % loopEnd), TICK);
    return () => clearInterval(interval);
  }, [visible, loopEnd]);

  return { frameRef, time };
}

function Chat({ time, schedule }: { time: number; schedule: Timeline }) {
  const messages = useMessages();
  const { message, steps, reply } = messages.agentWindow;
  const t = messages.wuWindow.chat;
  const sent = time >= schedule.sentAt;
  const replied = time >= schedule.replyAt;
  const draft = sent ? '' : message.slice(0, Math.max(0, Math.floor((time - TYPE_START) / schedule.typeSpeed)));
  const shownSteps = steps.slice(0, schedule.stepTimes.filter((at) => time >= at).length);
  const working = sent && !replied;
  const elapsed = Math.floor((time - schedule.sentAt) / 1000);

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-[#181a21]">
      <div className="flex min-h-0 flex-1 flex-col justify-end overflow-hidden">
        <div className="flex flex-col px-12 pt-[26px] pb-8">
          {sent && (
            <div className="flex flex-col items-end">
              <div className="max-w-[80%] rounded-[16px] bg-[#ebebeb14] px-4 py-2.5 text-[14px] leading-[22px]" style={{ color: text }}>
                {message}
              </div>
              <div className="h-8" />
            </div>
          )}
          {shownSteps.length > 0 && (
            <div className="pt-3">
              <div className="flex h-[26px] items-center gap-1.5 text-[12px] leading-[18px]" style={{ color: muted }}>
                <span className="relative h-[18px] w-[22px]">
                  <span className={`absolute top-0.5 left-[5.5px] transition-transform ${working ? '' : '-rotate-90'}`}>
                    <Icon name="agent_arrow_down" size={14} />
                  </span>
                </span>
                <span className={working ? 'text-shimmer' : ''}>{stepsSummary(shownSteps, messages.wuWindow.summary)}</span>
              </div>
              {working &&
                shownSteps.map((step, index) => (
                  <StepRow
                    key={index}
                    step={step}
                    isLast={index === shownSteps.length - 1}
                    expanded={false}
                    onToggle={() => {}}
                  />
                ))}
            </div>
          )}
          {working && (
            <div className="flex items-center gap-2 pt-4 text-[11px]">
              <WorkingSpinner />
              <span className="text-[12px]" style={{ color: muted }}>
                {t.working}
              </span>
              <span className="relative top-px text-[#aaa8b780]">{elapsed}s</span>
            </div>
          )}
          {replied && (
            <div className="animate-fade-in pt-3">
              <AssistantText source={reply} />
              <div className="h-8" />
            </div>
          )}
        </div>
      </div>

      <div className="w-full px-4 pb-2">
        <div className="flex h-[47px] items-center rounded-[22px] border border-[#2c2f3a] bg-[#2a2d37] pr-2 pl-2 shadow-[0_10px_15px_-3px_rgba(0,0,0,0.1),0_4px_6px_-4px_rgba(0,0,0,0.1)]">
          <span className="grid size-7 shrink-0 place-items-center">
            <Icon name="agent_paperclip" size={18} color={muted} />
          </span>
          <span className="min-w-0 flex-1 truncate px-2 text-[14px]" style={{ color: draft ? text : placeholder }}>
            {draft || t.placeholder}
            {!sent && time >= TYPE_START && (
              <span className="ml-px inline-block h-[17px] w-px translate-y-[3px]" style={{ background: text }} />
            )}
          </span>
          <span className="flex h-8 shrink-0 items-center gap-1.5 rounded-[8px] px-1.5 text-[12px] font-medium text-[#f0eef4e6]">
            <Icon name="agent_claude" size={16} color={claude} />
            Opus 5.5
            <span className="text-[#aaa8b7b3]">{t.effort}</span>
          </span>
          <span className="flex shrink-0 items-center gap-2 pl-0.5">
            <span className="grid size-7 place-items-center">
              <Icon name="agent_microphone" size={18} color={muted} />
            </span>
            <span className={`grid size-7 place-items-center rounded-full bg-[#f0eef4] ${draft || working ? '' : 'opacity-35'}`}>
              {working ? (
                <span className="size-[11px] rounded-[3px] bg-[#181a21]" />
              ) : (
                <Icon name="agent_send" size={14} color="#181a21" />
              )}
            </span>
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
            <UsageRing percent={sent ? 21 : 18} color={accent} />
            <UsageRing percent={34} color={muted} />
          </span>
        </div>
      </div>
    </div>
  );
}

export function AgentWindow() {
  const messages = useMessages();
  const { message, chatTitle } = messages.agentWindow;
  const schedule = timeline(message);
  const { frameRef, time } = useLoopTime(schedule.loopEnd);
  const title = time >= schedule.sentAt ? chatTitle : messages.wuWindow.tabs.newChat;

  return (
    <div ref={frameRef} className={`${mediaFrame} grid card-raised aspect-[16/10] place-items-center`} aria-hidden="true" inert>
      <div className="@container w-[88%] overflow-hidden rounded-[10px] shadow-[0_24px_60px_rgba(0,0,0,0.4),inset_0_0_0_1px_rgba(255,255,255,0.12)]" style={{ aspectRatio: `${WIDTH} / ${HEIGHT}` }}>
        <div
          className="flex origin-top-left flex-col bg-[#181a21] font-wu text-[14px] tracking-normal select-none"
          style={{ width: WIDTH, height: HEIGHT, color: text, scale: `tan(atan2(100cqw, ${WIDTH}px))` }}
        >
          <div className="flex h-8 shrink-0 items-center gap-1 border-b border-[#2c2f3a] px-2">
            <div className="flex h-6 max-w-[320px] items-center gap-[3px] rounded-[6px] bg-[#22252e] px-1 text-[14px]">
              <span className="grid size-[18px] place-items-center">
                <Icon name="agent_claude" size={14} color={claude} />
              </span>
              <span className="truncate pr-1">{title}</span>
            </div>
          </div>
          <Chat time={time} schedule={schedule} />
        </div>
      </div>
    </div>
  );
}
