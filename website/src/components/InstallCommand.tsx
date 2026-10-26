import { Check, Copy } from 'lucide-react';
import { useEffect, useState } from 'react';
import { INSTALL_COMMAND } from '../consts';
import { useMessages } from '../i18n/locale';

export function InstallCommand({ className = '' }: { className?: string }) {
  const t = useMessages().installCommand;
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timeout = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(timeout);
  }, [copied]);

  const copy = () => {
    navigator.clipboard.writeText(INSTALL_COMMAND).then(
      () => setCopied(true),
      (error) => console.debug('Could not copy the install command', error)
    );
  };

  return (
    <div
      className={`flex max-w-full min-h-[46px] items-center gap-2 rounded-full bg-black/40 py-1 pr-1 pl-[18px] ring-1 ring-elevated/10 ${className}`}
    >
      <code className="min-w-0 overflow-x-auto rounded-none bg-transparent p-0 text-[15px] whitespace-nowrap">
        <span className="text-tertiary select-none">$ </span>
        {INSTALL_COMMAND}
      </code>
      <button
        type="button"
        className="btn size-[38px] shrink-0 btn-ghost text-secondary hover:text-primary"
        onClick={copy}
        aria-label={copied ? t.copied : t.copyLabel}
        title={copied ? t.copied : t.copy}
      >
        {copied ? <Check size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}
      </button>
    </div>
  );
}
