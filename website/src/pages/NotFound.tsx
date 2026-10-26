import { Footer } from '../components/Footer';
import { Header } from '../components/Header';
import { sectionBody } from '../components/Section';
import { PageContext, localePath, messagesFor, type Locale } from '../i18n/locale';

export function NotFound({ locale }: { locale: Locale }) {
  const t = messagesFor(locale).notFound;
  return (
    <PageContext value={{ locale, path: '/' }}>
      <Header />
      <main className="mx-auto flex max-w-[600px] flex-col items-center gap-4 px-8 pt-48 pb-16 text-center max-sm:px-4 max-sm:pt-36">
        <h1 className="text-[clamp(36px,5vw,48px)] leading-[1.1] font-medium tracking-[-0.03em]">{t.title}</h1>
        <p className={sectionBody}>{t.body}</p>
        <a className="btn btn-md btn-primary mt-2" href={localePath(locale, '/')}>
          {t.back}
        </a>
      </main>
      <Footer />
    </PageContext>
  );
}
