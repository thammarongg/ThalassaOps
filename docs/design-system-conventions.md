# Design-system conventions

All visible UI copy in `ui/src` is accessed through `useTranslation()` and the `ui/src/locales/en.ts` and `ui/src/locales/th.ts` catalogs. The `local/no-user-facing-jsx-text` ESLint rule rejects direct prose in production JSX; tests and translation resources are excluded so assertions and catalogs remain readable.

The rule was introduced with this Sprint 3 change and verified by running lint against a temporary JSX literal, observing the expected error, then removing the literal and running the project lint command successfully.
