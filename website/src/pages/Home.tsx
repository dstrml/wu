import { ArrowUpRight, BookOpen, Download } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Faq } from '../components/Faq';
import { AgentWindow } from '../components/AgentWindow';
import { Footer } from '../components/Footer';
import { Header } from '../components/Header';
import { AppIcon, GithubIcon } from '../components/icons';
import { HeroBackground } from '../components/HeroBackground';
import { InstallCommand } from '../components/InstallCommand';
import { MemoryChart } from '../components/MemoryChart';
import { RichText } from '../components/RichText';
import { CenterSection, MediaSection, frameOutline, mediaFrame, sectionBody, sectionTitle } from '../components/Section';
import { WuWindowFrame } from '../components/WuWindow';
import { MEMORY_BENCHMARK_URL, REPO_URL } from '../consts';
import {
  detectMacArchitecture,
  detectPlatform,
  directDownload,
  downloads,
  isSuggested,
  platformNames,
  type MacArchitecture,
  type Platform
} from '../data/downloads';
import { PageContext, localePath, messagesFor, type Locale } from '../i18n/locale';

export function Home({ locale }: { locale: Locale }) {
  const messages = messagesFor(locale);
  const t = messages.home;
  const [platform, setPlatform] = useState<Platform | null>(null);
  const [macArchitecture, setMacArchitecture] = useState<MacArchitecture>('arm');

  useEffect(() => {
    const detected = detectPlatform();
    setPlatform(detected);
    if (detected === 'mac') detectMacArchitecture().then(setMacArchitecture);
  }, []);

  const direct = directDownload(platform, macArchitecture);
  const downloadHref = direct?.href ?? '#download';
  const hasInstallScript = platform === 'mac' || platform === 'linux';

  const downloadButtons = (highlightSuggested: boolean) => (
    <div className="mt-2 flex flex-wrap justify-center gap-2">
      {downloads.map((download) => (
        <a
          key={download.id}
          className={`btn btn-md ${highlightSuggested && isSuggested(download, platform, macArchitecture) ? 'btn-light' : 'btn-soft'}`}
          href={download.href}
        >
          {messages.downloads[download.id]}
        </a>
      ))}
    </div>
  );

  return (
    <PageContext value={{ locale, path: '/' }}>
      <Header downloadHref={hasInstallScript ? '#download' : downloadHref} />
      <main>
        <div className="relative isolate m-1 mb-40 overflow-hidden rounded-xl max-sm:mb-18">
          <div
            aria-hidden="true"
            className="absolute inset-0 -z-10 bg-linear-to-b from-hero-start to-hero-end"
            style={{ maskImage: 'linear-gradient(to bottom, black 40%, transparent)' }}
          >
            <HeroBackground />
          </div>
          <section className="px-8 pt-42 pb-26 max-sm:px-4 max-sm:pt-30 max-sm:pb-14">
            <div className="mx-auto flex max-w-[1240px] flex-col items-start gap-5">
              <h1 className="max-w-[800px] text-[clamp(36px,5.2vw,60px)] leading-[1.1] font-medium tracking-[-0.03em]">
                {t.title}
              </h1>
              <p className="max-w-[600px] text-xl leading-[1.35] max-sm:text-lg">{t.tagline}</p>
              {hasInstallScript ? (
                <div className="mt-2 flex max-w-full flex-col items-start gap-3">
                  <InstallCommand />
                  <p className="pl-[18px] text-sm text-tertiary">
                    <RichText source={t.installManually(downloadHref)} />
                  </p>
                </div>
              ) : (
                <div className="mt-2 flex flex-wrap gap-2">
                  <a className="btn btn-lg btn-light" href={downloadHref}>
                    <Download size={18} aria-hidden="true" />
                    {direct && platform ? t.downloadFor(platformNames[platform]) : t.downloadWu}
                  </a>
                  <a className="btn btn-lg btn-primary" href="#download">
                    {t.allPlatforms}
                  </a>
                </div>
              )}
            </div>
          </section>

          <section className="mx-auto mt-28 max-w-[1240px] px-8 max-sm:mt-18 max-sm:px-4">
            <WuWindowFrame className={`${mediaFrame} ${frameOutline}`} />
          </section>
        </div>

        <MediaSection
          mediaFirst
          text={
            <>
              <h2 className={sectionTitle}>{t.memory.title}</h2>
              <p className={sectionBody}>{t.memory.body}</p>
              <a className="btn btn-md btn-light mt-2" href={MEMORY_BENCHMARK_URL} target="_blank" rel="noopener">
                {t.memory.link}
                <ArrowUpRight size={18} aria-hidden="true" />
              </a>
            </>
          }
          media={<MemoryChart />}
        />

        <MediaSection
          text={
            <>
              <h2 className={sectionTitle}>{t.agents.title}</h2>
              <p className={sectionBody}>{t.agents.body}</p>
              <a className="btn btn-md btn-soft mt-2" href={localePath(locale, '/docs/#agent-chat')}>
                {t.agents.link}
              </a>
            </>
          }
          media={<AgentWindow />}
        />

        <CenterSection>
          <h2 className={sectionTitle}>{t.openSource.title}</h2>
          <p className={sectionBody}>{t.openSource.body}</p>
          <div className="mt-2 flex flex-wrap justify-center gap-2">
            <a className="btn btn-md btn-light" href={REPO_URL} target="_blank" rel="noopener">
              <GithubIcon />
              {t.openSource.source}
            </a>
            <a className="btn btn-md btn-primary" href={localePath(locale, '/docs/')}>
              <BookOpen size={18} aria-hidden="true" />
              {t.openSource.docs}
            </a>
          </div>
        </CenterSection>

        <CenterSection id="download">
          <AppIcon className="mb-2 size-24 rounded-[24%]" alt={t.ready.iconAlt} />
          <h2 className={sectionTitle}>{t.ready.title}</h2>
          <p className={`${sectionBody} max-w-[520px]`}>{hasInstallScript ? t.ready.bodyScript : t.ready.bodyPick}</p>
          {hasInstallScript ? (
            <>
              <InstallCommand className="mt-2" />
              <p className="mt-6 text-sm text-tertiary">{t.ready.orInstallManually}</p>
              {downloadButtons(false)}
            </>
          ) : (
            <>
              {downloadButtons(true)}
              {platform !== 'windows' && (
                <>
                  <p className="mt-6 text-sm text-tertiary">{t.ready.alsoRun}</p>
                  <InstallCommand className="mt-2" />
                </>
              )}
            </>
          )}
          <p className="mt-2 text-sm leading-[1.4] text-tertiary">
            <RichText source={t.ready.firstTime} />
          </p>
        </CenterSection>

        <Faq />
      </main>
      <Footer />
    </PageContext>
  );
}
