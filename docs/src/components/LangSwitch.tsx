import Link from "next/link";
import { localeLabels, locales, type Locale, ui } from "@/lib/i18n";

type Props = {
  locale: Locale;
  slug: string;
};

export function LangSwitch({ locale, slug }: Props) {
  const path = slug ? `/${slug}` : "";
  return (
    <div className="lang-switch" aria-label={ui[locale].language}>
      {locales.map((loc) => (
        <Link
          key={loc}
          href={`/${loc}${path}`}
          className={loc === locale ? "lang-active" : "lang-idle"}
          hrefLang={loc}
        >
          {localeLabels[loc]}
        </Link>
      ))}
    </div>
  );
}
