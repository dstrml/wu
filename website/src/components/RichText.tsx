const inlinePattern = /(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\([^)]+\))/;

export function RichText({ source }: { source: string }) {
  return (
    <>
      {source.split(inlinePattern).map((part, index) => {
        if (part.startsWith('`') && part.endsWith('`')) return <code key={index}>{part.slice(1, -1)}</code>;
        if (part.startsWith('**') && part.endsWith('**')) return <strong key={index}>{part.slice(2, -2)}</strong>;
        const link = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(part);
        if (link) {
          const [, label, href = ''] = link;
          return (
            <a key={index} href={href} {...(href.startsWith('http') && { target: '_blank', rel: 'noopener' })}>
              {label}
            </a>
          );
        }
        return part;
      })}
    </>
  );
}
