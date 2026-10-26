import { Footer } from '../components/Footer';
import { Header } from '../components/Header';
import { RichText } from '../components/RichText';
import type { Messages } from '../i18n/en';
import { PageContext, messagesFor, type Locale } from '../i18n/locale';

const paths: { id: keyof Messages['docs']['gettingStarted']['paths']; path: string }[] = [
  { id: 'commandLine', path: 'wu' },
  { id: 'settingsUnix', path: '~/.config/wu' },
  { id: 'settingsWindows', path: '%APPDATA%\\Wu' },
  { id: 'appDataMac', path: '~/Library/Application Support/Wu' },
  { id: 'projectSettings', path: '.wu/' }
];

function List({ items }: { items: string[] }) {
  return (
    <ul>
      {items.map((item) => (
        <li key={item}>
          <RichText source={item} />
        </li>
      ))}
    </ul>
  );
}

export function Docs({ locale }: { locale: Locale }) {
  const t = messagesFor(locale).docs;
  return (
    <PageContext value={{ locale, path: '/docs/' }}>
      <Header />
      <main className="doc mx-auto max-w-[760px] px-8 pt-32 pb-16 max-sm:px-4">
        <h1>{t.title}</h1>

        <div className="mb-6 rounded-xl card-raised px-5 py-[18px] text-[17px] leading-normal text-secondary [&_a]:text-primary [&_a]:underline [&_a]:underline-offset-2">
          <RichText source={t.intro} />
        </div>

        <h2>{t.gettingStarted.title}</h2>
        <p>
          <RichText source={t.gettingStarted.body} />
        </p>
        <div className="overflow-x-auto">
          <table>
            <tbody>
              {paths.map((row) => (
                <tr key={row.id}>
                  <td>{t.gettingStarted.paths[row.id]}</td>
                  <td>
                    <code>{row.path}</code>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <h2 id="agent-chat">{t.agentChat.title}</h2>
        <p>{t.agentChat.intro}</p>
        <List items={t.agentChat.items} />
        <p>{t.agentChat.outro}</p>

        <h2>{t.workspace.title}</h2>
        <List items={t.workspace.items} />

        <h2>{t.extensions.title}</h2>
        <p>{t.extensions.body}</p>

        <h2>{t.privacy.title}</h2>
        <p>{t.privacy.body}</p>

        <h2>{t.notIncluded.title}</h2>
        <List items={t.notIncluded.items} />
        <p className="text-tertiary!">{t.notIncluded.note}</p>

        <h2>{t.comingFromZed.title}</h2>
        <p>
          <RichText source={t.comingFromZed.body} />
        </p>

        <h2>{t.raycast.title}</h2>
        <p>
          <RichText source={t.raycast.body} />
        </p>

        <h2>{t.license.title}</h2>
        <p>{t.license.body}</p>
      </main>
      <Footer />
    </PageContext>
  );
}
