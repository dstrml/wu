import { Heart } from 'lucide-react';
import { Footer } from '../components/Footer';
import { Header } from '../components/Header';
import { sectionBody } from '../components/Section';
import { GithubIcon } from '../components/icons';
import { ISSUES_URL, OPEN_COLLECTIVE_URL, REPO_URL } from '../consts';
import { PageContext, messagesFor, type Locale } from '../i18n/locale';

export function Support({ locale }: { locale: Locale }) {
  const t = messagesFor(locale).support;
  return (
    <PageContext value={{ locale, path: '/support/' }}>
      <Header />
      <main className="mx-auto flex max-w-[640px] flex-col items-center px-8 pt-44 pb-16 text-center max-sm:px-4 max-sm:pt-32">
        <h1 className="text-[clamp(36px,5vw,48px)] leading-[1.1] font-medium tracking-[-0.03em]">{t.title}</h1>
        <div className="mt-8 flex flex-col gap-5">
          <p className={`${sectionBody} text-balance`}>{t.intro}</p>
          <p className={`${sectionBody} text-balance`}>{t.why}</p>
        </div>
        <a className="btn btn-lg btn-light mt-12" href={OPEN_COLLECTIVE_URL} target="_blank" rel="noopener">
          <Heart size={18} aria-hidden="true" />
          {t.donate}
        </a>
        <p className="mt-5 text-sm leading-[1.5] text-balance text-tertiary opacity-60">{t.donateNote}</p>

        <div className="mt-24 w-full rounded-xl card-raised px-8 py-9 max-sm:mt-16 max-sm:px-5">
          <h2 className="text-xl font-medium">{t.otherWays.title}</h2>
          <p className="mt-3 text-[17px] leading-normal text-balance text-secondary">{t.otherWays.body}</p>
          <div className="mt-6 flex flex-wrap justify-center gap-2">
            <a className="btn btn-md btn-soft" href={REPO_URL} target="_blank" rel="noopener">
              <GithubIcon />
              {t.otherWays.star}
            </a>
            <a className="btn btn-md btn-soft" href={ISSUES_URL} target="_blank" rel="noopener">
              {t.otherWays.reportIssue}
            </a>
          </div>
        </div>
      </main>
      <Footer />
    </PageContext>
  );
}
