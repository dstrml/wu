import type { ReactElement } from 'react';
import type { Messages } from './i18n/en';
import { localePath, locales, messagesFor, type Locale } from './i18n/locale';
import { Docs } from './pages/Docs';
import { Home } from './pages/Home';
import { NotFound } from './pages/NotFound';
import { Support } from './pages/Support';

export type ClientEntry = 'home';

export interface Route {
  path: string;
  locale: Locale;
  element: ReactElement;
  entry?: ClientEntry;
  alternates?: Record<Locale, string>;
  meta?: {
    title?: string;
    description?: string;
  };
}

interface Page {
  basePath: string;
  render: (locale: Locale) => ReactElement;
  meta: keyof Messages['meta'];
  entry?: ClientEntry;
}

const pages: Page[] = [
  { basePath: '', render: (locale) => <Home locale={locale} />, meta: 'home', entry: 'home' },
  { basePath: '/docs', render: (locale) => <Docs locale={locale} />, meta: 'docs' },
  { basePath: '/support', render: (locale) => <Support locale={locale} />, meta: 'support' }
];

const routePath = (locale: Locale, basePath: string) => localePath(locale, basePath) || '/';

const translatedRoutes = locales.flatMap((locale) =>
  pages.map(
    (page): Route => ({
      path: routePath(locale, page.basePath),
      locale,
      element: page.render(locale),
      entry: page.entry,
      alternates: Object.fromEntries(
        locales.map((alternate) => [alternate, routePath(alternate, page.basePath)])
      ) as Record<Locale, string>,
      meta: messagesFor(locale).meta[page.meta]
    })
  )
);

export const routes: Route[] = [
  ...translatedRoutes,
  { path: '/404', locale: 'en', element: <NotFound locale="en" />, meta: messagesFor('en').meta.notFound }
];
