import { notFound } from "next/navigation";
import { DocsShell } from "@/components/DocsShell";
import { Markdown } from "@/components/Markdown";
import { getDoc, getNeighbors, listDocSlugs } from "@/lib/content";
import { isLocale, locales } from "@/lib/i18n";
import { nav } from "@/lib/nav";

export function generateStaticParams() {
  const params: { locale: string; slug: string }[] = [];
  for (const locale of locales) {
    for (const slug of listDocSlugs(locale)) {
      if (slug) params.push({ locale, slug });
    }
    // Fallback from nav if content not yet listed
    for (const item of nav) {
      if (item.slug && !params.some((p) => p.locale === locale && p.slug === item.slug)) {
        params.push({ locale, slug: item.slug });
      }
    }
  }
  return params;
}

export default async function DocPage({
  params,
}: PageProps<"/[locale]/[slug]">) {
  const { locale, slug } = await params;
  if (!isLocale(locale)) notFound();
  const doc = getDoc(locale, slug);
  if (!doc) notFound();
  return (
    <DocsShell
      locale={locale}
      slug={slug}
      title={doc.title}
      description={doc.description}
      neighbors={getNeighbors(locale, slug)}
    >
      <Markdown content={doc.body} locale={locale} />
    </DocsShell>
  );
}
