import Link from "next/link";
import { LangSwitch } from "@/components/LangSwitch";
import { Sidebar } from "@/components/Sidebar";
import { ui, type Locale } from "@/lib/i18n";

type Neighbor = { slug: string; title: string } | null;

type Props = {
  locale: Locale;
  slug: string;
  title: string;
  description?: string;
  neighbors: { prev: Neighbor; next: Neighbor };
  children: React.ReactNode;
};

export function DocsShell({
  locale,
  slug,
  title,
  description,
  neighbors,
  children,
}: Props) {
  const t = ui[locale];
  return (
    <div className="docs-root">
      <div className="docs-atmosphere" aria-hidden />
      <header className="docs-topbar">
        <Link href={`/${locale}`} className="top-brand font-display">
          {t.brand}
        </Link>
        <p className="top-tagline">{t.tagline}</p>
        <LangSwitch locale={locale} slug={slug} />
      </header>
      <div className="docs-frame">
        <Sidebar locale={locale} currentSlug={slug} />
        <main className="docs-main">
          <article className="docs-article">
            <header className="article-head">
              <p className="article-kicker">{t.docs}</p>
              <h1 className="article-title font-display">{title}</h1>
              {description ? (
                <p className="article-desc">{description}</p>
              ) : null}
            </header>
            <div className="article-body">{children}</div>
            <footer className="article-nav">
              {neighbors.prev ? (
                <Link
                  href={
                    neighbors.prev.slug
                      ? `/${locale}/${neighbors.prev.slug}`
                      : `/${locale}`
                  }
                  className="pager prev"
                >
                  <span>{t.prev}</span>
                  <strong>{neighbors.prev.title}</strong>
                </Link>
              ) : (
                <span />
              )}
              {neighbors.next ? (
                <Link
                  href={
                    neighbors.next.slug
                      ? `/${locale}/${neighbors.next.slug}`
                      : `/${locale}`
                  }
                  className="pager next"
                >
                  <span>{t.next}</span>
                  <strong>{neighbors.next.title}</strong>
                </Link>
              ) : null}
            </footer>
          </article>
        </main>
      </div>
    </div>
  );
}
