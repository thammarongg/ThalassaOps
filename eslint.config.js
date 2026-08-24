import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

const noUserFacingJsxText = {
  meta: {
    type: "problem",
    messages: { localized: "Route user-facing JSX text through the translation catalog." }
  },
  create(context) {
    return {
      JSXText(node) {
        if (/[^\s\p{P}\p{S}\p{N}]/u.test(node.value))
          context.report({ node, messageId: "localized" });
      }
    };
  }
};
export default tseslint.config(
  { ignores: ["ui/dist/**"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["ui/**/*.{ts,tsx}"],
    ignores: ["ui/**/*.test.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
      local: { rules: { "no-user-facing-jsx-text": noUserFacingJsxText } }
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": "off",
      "local/no-user-facing-jsx-text": "error"
    }
  }
);
