import { notFound } from "next/navigation";
import { DocsShell } from "@/components/DocsShell";
import { Markdown } from "@/components/Markdown";
import { getDoc, getNeighbors } from "@/lib/content";
import { isLocale } from "@/lib/i18n";

export default async function LocaleHomePage({
  params,
}: PageProps<"/[locale]">) {
  const { locale } = await params;
  if (!isLocale(locale)) notFound();
  const doc = getDoc(locale, "");
  if (!doc) notFound();
  return (
    <DocsShell
      locale={locale}
      slug=""
      title={doc.title}
      description={doc.description}
      neighbors={getNeighbors(locale, "")}
    >
      <Markdown content={doc.body} locale={locale} />
    </DocsShell>
  );
}
