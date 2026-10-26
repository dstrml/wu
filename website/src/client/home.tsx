import { hydrateRoot } from 'react-dom/client';
import { isLocale } from '../i18n/locale';
import { Home } from '../pages/Home';

const root = document.getElementById('root');
const lang = document.documentElement.lang;
if (root) hydrateRoot(root, <Home locale={isLocale(lang) ? lang : 'en'} />);
