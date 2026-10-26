import { ChevronDown } from 'lucide-react';
import { useMessages } from '../i18n/locale';
import { RichText } from './RichText';

export function Faq() {
  const t = useMessages().faq;
  return (
    <section className="mx-auto my-28 flex max-w-[1240px] flex-col items-center gap-3 px-8 max-sm:my-18 max-sm:px-4">
      <p className="text-lg text-secondary">{t.title}</p>
      <div className="grid w-[min(760px,100%)] gap-1.5">
        {t.questions.map(({ question, answer }) => (
          <details key={question} className="group rounded-xl card-raised">
            <summary className="flex cursor-pointer list-none items-center justify-between gap-2 rounded-xl px-5 py-[18px] text-[17px] outline-offset-2 outline-primary select-none focus-visible:outline-2 max-sm:px-[18px] max-sm:py-4 max-sm:text-base [&::-webkit-details-marker]:hidden">
              {question}
              <ChevronDown
                size={18}
                className="shrink-0 text-secondary transition-transform duration-200 group-open:rotate-180"
                aria-hidden="true"
              />
            </summary>
            <p className="pr-12 pb-[18px] pl-5 leading-[1.45] text-secondary max-sm:px-[18px] max-sm:pb-4">
              <RichText source={answer} />
            </p>
          </details>
        ))}
      </div>
    </section>
  );
}
