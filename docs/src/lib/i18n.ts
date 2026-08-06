export const locales = ["pt", "en", "es"] as const;
export type Locale = (typeof locales)[number];
export const defaultLocale: Locale = "pt";

export function isLocale(value: string): value is Locale {
  return (locales as readonly string[]).includes(value);
}

export const localeLabels: Record<Locale, string> = {
  pt: "Português",
  en: "English",
  es: "Español",
};

export const ui = {
  pt: {
    brand: "Steampunk",
    tagline: "Linguagem compilada, tipada e async-first",
    docs: "Documentação",
    onThisPage: "Nesta página",
    prev: "Anterior",
    next: "Próximo",
    language: "Idioma",
    searchHint: "Navegue pelas seções à esquerda",
    mvpBadge: "MVP v0.1",
    openMenu: "Abrir menu",
    closeMenu: "Fechar menu",
    home: "Início",
  },
  en: {
    brand: "Steampunk",
    tagline: "Compiled, statically typed, async-first language",
    docs: "Documentation",
    onThisPage: "On this page",
    prev: "Previous",
    next: "Next",
    language: "Language",
    searchHint: "Browse sections in the sidebar",
    mvpBadge: "MVP v0.1",
    openMenu: "Open menu",
    closeMenu: "Close menu",
    home: "Home",
  },
  es: {
    brand: "Steampunk",
    tagline: "Lenguaje compilado, tipado y async-first",
    docs: "Documentación",
    onThisPage: "En esta página",
    prev: "Anterior",
    next: "Siguiente",
    language: "Idioma",
    searchHint: "Navega por las secciones a la izquierda",
    mvpBadge: "MVP v0.1",
    openMenu: "Abrir menú",
    closeMenu: "Cerrar menú",
    home: "Inicio",
  },
} as const;
