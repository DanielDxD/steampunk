import Link from "next/link";
import { ui, type Locale } from "@/lib/i18n";
import { hrefFor, nav } from "@/lib/nav";

type Props = {
  locale: Locale;
  currentSlug: string;
};

export function Sidebar({ locale, currentSlug }: Props) {
  const t = ui[locale];
  return (
    <aside className="docs-sidebar">
      <div className="sidebar-brand">
        <Link href={`/${locale}`} className="brand-mark">
          <span className="brand-gear" aria-hidden>
            ⌘
          </span>
          <span>
            <span className="brand-name font-display">{t.brand}</span>
            <span className="brand-sub">{t.docs}</span>
          </span>
        </Link>
        <span className="mvp-pill">{t.mvpBadge}</span>
      </div>
      <p className="sidebar-hint">{t.searchHint}</p>
      <nav className="sidebar-nav" aria-label={t.docs}>
        {nav.map((item) => {
          const active = item.slug === currentSlug;
          return (
            <Link
              key={item.slug || "index"}
              href={hrefFor(locale, item.slug)}
              className={active ? "nav-link active" : "nav-link"}
            >
              {item.title[locale]}
            </Link>
          );
        })}
      </nav>
    </aside>
  );
}
