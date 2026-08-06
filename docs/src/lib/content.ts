import fs from "node:fs";
import path from "node:path";
import matter from "gray-matter";
import type { Locale } from "./i18n";
import { nav } from "./nav";

const contentRoot = path.join(process.cwd(), "content");

export type DocPage = {
  locale: Locale;
  slug: string;
  title: string;
  description?: string;
  body: string;
};

export function getDoc(locale: Locale, slug: string): DocPage | null {
  const fileSlug = slug === "" ? "index" : slug;
  const filePath = path.join(contentRoot, locale, `${fileSlug}.md`);
  if (!fs.existsSync(filePath)) return null;
  const raw = fs.readFileSync(filePath, "utf8");
  const { data, content } = matter(raw);
  return {
    locale,
    slug,
    title: String(data.title ?? fileSlug),
    description: data.description ? String(data.description) : undefined,
    body: content.trim(),
  };
}

export function listDocSlugs(locale: Locale): string[] {
  const dir = path.join(contentRoot, locale);
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".md"))
    .map((f) => (f === "index.md" ? "" : f.replace(/\.md$/, "")));
}

export function getNeighbors(locale: Locale, slug: string) {
  const idx = nav.findIndex((item) => item.slug === slug);
  if (idx < 0) return { prev: null, next: null };
  const prev = idx > 0 ? nav[idx - 1] : null;
  const next = idx < nav.length - 1 ? nav[idx + 1] : null;
  return {
    prev: prev
      ? { slug: prev.slug, title: prev.title[locale] }
      : null,
    next: next
      ? { slug: next.slug, title: next.title[locale] }
      : null,
  };
}
