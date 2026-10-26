import { Download } from 'lucide-react';
import { Fragment } from 'react';
import { REPO_URL } from '../consts';
import { localePath, locales, messagesFor, useMessages, usePage } from '../i18n/locale';
import { AppIcon } from './icons';

const glassPill =
  'pointer-events-auto rounded-full bg-background/72 ring-1 ring-elevated/10 backdrop-blur-md backdrop-saturate-110';

export function LanguageSwitch({ className = '', onlyOther = false }: { className?: string; onlyOther?: boolean }) {
  const { locale, path } = usePage();
  const shown = onlyOther ? locales.filter((candidate) => candidate !== locale) : locales;
  return shown.map((candidate, index) => (
    <Fragment key={candidate}>
      {index > 0 && ' / '}
      {candidate === locale ? (
        <span lang={candidate} aria-current="true">
          {messagesFor(candidate).languageName}
        </span>
      ) : (
        <a className={className} href={localePath(candidate, path)} lang={candidate} hrefLang={candidate}>
          {messagesFor(candidate).languageName}
        </a>
      )}
    </Fragment>
  ));
}

export function Header({ downloadHref }: { downloadHref?: string }) {
  const { locale } = usePage();
  const t = useMessages().header;
  return (
    <header className="pointer-events-none fixed inset-x-0 top-0 z-10 px-8 pt-4 max-sm:px-4 max-sm:pt-2.5">
      <div className="mx-auto flex max-w-[1240px] items-center justify-between gap-2">
        <a
          className={`${glassPill} inline-flex items-center gap-2.5 py-[5px] pr-4 pl-[5px] text-lg font-semibold no-underline`}
          href={localePath(locale, '/')}
        >
          <AppIcon className="size-8 rounded-full" />
          Wu
        </a>
        <nav className={`${glassPill} flex gap-0.5 p-1`}>
          <a className="btn btn-md btn-ghost" href={localePath(locale, '/docs/')}>
            {t.docs}
          </a>
          {/* <a className="btn btn-md btn-ghost max-sm:hidden" href={localePath(locale, '/support/')}>
            {t.support}
          </a> */}
          <a className="btn btn-md btn-ghost max-sm:hidden" href={REPO_URL} target="_blank" rel="noopener">
            GitHub
          </a>
          <LanguageSwitch className="btn btn-md btn-ghost" onlyOther />
          <a className="btn btn-md btn-light" href={downloadHref ?? localePath(locale, '/#download')}>
            <Download size={18} aria-hidden="true" />
            {t.download}
          </a>
        </nav>
      </div>
    </header>
  );
}
