import type { Locale } from "./i18n";

export type NavItem = {
  slug: string;
  title: Record<Locale, string>;
};

export const nav: NavItem[] = [
  {
    slug: "",
    title: {
      pt: "Visão geral",
      en: "Overview",
      es: "Visión general",
    },
  },
  {
    slug: "getting-started",
    title: {
      pt: "Começando",
      en: "Getting started",
      es: "Primeros pasos",
    },
  },
  {
    slug: "basics",
    title: {
      pt: "Sintaxe básica",
      en: "Language basics",
      es: "Sintaxis básica",
    },
  },
  {
    slug: "functions",
    title: {
      pt: "Funções e closures",
      en: "Functions & closures",
      es: "Funciones y closures",
    },
  },
  {
    slug: "control-flow",
    title: {
      pt: "Controle de fluxo",
      en: "Control flow",
      es: "Control de flujo",
    },
  },
  {
    slug: "modules",
    title: {
      pt: "Módulos e imports",
      en: "Modules & imports",
      es: "Módulos e imports",
    },
  },
  {
    slug: "classes",
    title: {
      pt: "Classes e OOP",
      en: "Classes & OOP",
      es: "Clases y OOP",
    },
  },
  {
    slug: "async",
    title: {
      pt: "Async e Future",
      en: "Async & Future",
      es: "Async y Future",
    },
  },
  {
    slug: "concurrency",
    title: {
      pt: "Concorrência",
      en: "Concurrency",
      es: "Concurrencia",
    },
  },
  {
    slug: "result-option",
    title: {
      pt: "Result e Option",
      en: "Result & Option",
      es: "Result y Option",
    },
  },
  {
    slug: "stdlib",
    title: {
      pt: "Biblioteca padrão",
      en: "Standard library",
      es: "Biblioteca estándar",
    },
  },
  {
    slug: "examples",
    title: {
      pt: "Exemplos práticos",
      en: "Practical examples",
      es: "Ejemplos prácticos",
    },
  },
  {
    slug: "mvp",
    title: {
      pt: "Status do MVP",
      en: "MVP status",
      es: "Estado del MVP",
    },
  },
];

export function hrefFor(locale: Locale, slug: string): string {
  return slug ? `/${locale}/${slug}` : `/${locale}`;
}
