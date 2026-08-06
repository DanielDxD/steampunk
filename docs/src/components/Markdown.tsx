import Link from "next/link";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Locale } from "@/lib/i18n";

type Props = {
  content: string;
  locale: Locale;
};

function resolveHref(locale: Locale, href?: string): string | undefined {
  if (!href) return href;
  if (
    href.startsWith("http://") ||
    href.startsWith("https://") ||
    href.startsWith("#") ||
    href.startsWith("mailto:")
  ) {
    return href;
  }
  const cleaned = href.replace(/^\.\//, "").replace(/\.md$/, "");
  if (!cleaned || cleaned === "index") return `/${locale}`;
  return `/${locale}/${cleaned}`;
}

export function Markdown({ content, locale }: Props) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        h1: ({ children }) => (
          <h1 className="doc-h1 font-display tracking-tight">{children}</h1>
        ),
        h2: ({ children }) => (
          <h2 className="doc-h2 font-display tracking-tight">{children}</h2>
        ),
        h3: ({ children }) => (
          <h3 className="doc-h3 font-display tracking-tight">{children}</h3>
        ),
        p: ({ children }) => <p className="doc-p">{children}</p>,
        ul: ({ children }) => <ul className="doc-ul">{children}</ul>,
        ol: ({ children }) => <ol className="doc-ol">{children}</ol>,
        li: ({ children }) => <li className="doc-li">{children}</li>,
        a: ({ href, children }) => {
          const resolved = resolveHref(locale, href);
          if (!resolved) return <span>{children}</span>;
          if (resolved.startsWith("http") || resolved.startsWith("mailto:")) {
            return (
              <a href={resolved} className="doc-a" target="_blank" rel="noreferrer">
                {children}
              </a>
            );
          }
          return (
            <Link href={resolved} className="doc-a">
              {children}
            </Link>
          );
        },
        blockquote: ({ children }) => (
          <blockquote className="doc-quote">{children}</blockquote>
        ),
        table: ({ children }) => (
          <div className="doc-table-wrap">
            <table className="doc-table">{children}</table>
          </div>
        ),
        code: ({ className, children }) => {
          const text = String(children).replace(/\n$/, "");
          const isBlock = Boolean(className) || text.includes("\n");
          if (!isBlock) {
            return <code className="doc-inline-code">{children}</code>;
          }
          const lang = className?.replace("language-", "") ?? "";
          return (
            <div className="doc-code-block">
              {lang ? <div className="doc-code-lang">{lang}</div> : null}
              <pre>
                <code>{text}</code>
              </pre>
            </div>
          );
        },
        pre: ({ children }) => <>{children}</>,
        hr: () => <hr className="doc-hr" />,
        strong: ({ children }) => <strong className="font-semibold text-[var(--ink)]">{children}</strong>,
      }}
    >
      {content}
    </ReactMarkdown>
  );
}
