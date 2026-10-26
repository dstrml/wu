import {
  INSTALL_GUIDE_URL,
  ISSUES_URL,
  MEMORY_BENCHMARK_URL,
  RAYCAST_URL,
  RELEASES_URL,
  REPO_URL
} from '../consts';
import { localePath, useMessages, usePage } from '../i18n/locale';
import { LanguageSwitch } from './Header';
import { AppIcon } from './icons';

export function Footer() {
  const { locale } = usePage();
  const t = useMessages().footer;
  const columns = [
    {
      title: t.product.title,
      links: [
        { label: t.product.download, href: localePath(locale, '/#download') },
        { label: t.product.docs, href: localePath(locale, '/docs/') },
        { label: t.product.releases, href: RELEASES_URL }
      ]
    },
    {
      title: t.resources.title,
      links: [
        { label: t.resources.installGuide, href: INSTALL_GUIDE_URL },
        { label: t.resources.memoryBenchmark, href: MEMORY_BENCHMARK_URL },
        { label: t.resources.raycast, href: RAYCAST_URL }
      ]
    },
    {
      title: t.source.title,
      links: [
        { label: t.source.github, href: REPO_URL },
        { label: t.source.reportIssue, href: ISSUES_URL },
        // { label: t.source.support, href: localePath(locale, '/support/') }
      ]
    }
  ];

  return (
    <footer className="mx-auto flex max-w-[1240px] flex-col items-center gap-12 px-8 pt-24 pb-12 max-sm:px-4">
      <nav className="flex flex-wrap justify-center gap-x-[clamp(32px,10vw,120px)] gap-y-8" aria-label={t.navLabel}>
        {columns.map((column) => (
          <div key={column.title} className="flex min-w-[120px] flex-col gap-2.5">
            <div className="mb-1 text-sm text-tertiary">{column.title}</div>
            {column.links.map((link) => (
              <a
                key={link.label}
                className="text-[15px] text-secondary no-underline hover:underline"
                href={link.href}
                {...(link.href.startsWith('http') && { target: '_blank', rel: 'noopener' })}
              >
                {link.label}
              </a>
            ))}
          </div>
        ))}
      </nav>
      <div className="flex flex-col items-center gap-4">
        <div className="text-sm text-tertiary">
          <LanguageSwitch className="text-secondary no-underline hover:underline" />
        </div>
        <div className="flex items-center gap-2.5 text-center text-sm text-tertiary">
          <AppIcon className="size-7 rounded-[24%]" />
          {t.license}
        </div>
      </div>
    </footer>
  );
}
