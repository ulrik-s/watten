import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import react from 'eslint-plugin-react';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import jsxA11y from 'eslint-plugin-jsx-a11y';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default tseslint.config(
  {
    // Generated, vendored, or build output — never linted.
    ignores: [
      'dist',
      'pkg',
      'pkg-test',
      'coverage',
      'node_modules',
      'playwright-report',
      'test-results',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommendedTypeChecked,
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      parserOptions: {
        // Type-aware linting: resolve each file to its nearest tsconfig.
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
      globals: { ...globals.browser },
    },
    plugins: {
      react,
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
      'jsx-a11y': jsxA11y,
    },
    settings: { react: { version: 'detect' } },
    rules: {
      ...react.configs.recommended.rules,
      ...react.configs['jsx-runtime'].rules,
      ...reactHooks.configs.recommended.rules,
      ...jsxA11y.configs.recommended.rules,
      'react-refresh/only-export-components': [
        'warn',
        { allowConstantExport: true },
      ],
      // Allow intentionally-unused bindings when prefixed with `_`
      // (e.g. destructured tuple slots we don't need).
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
          destructuredArrayIgnorePattern: '^_',
        },
      ],
      // React event handlers are commonly `async`; allow passing a
      // promise-returning function to a void-returning JSX attribute.
      '@typescript-eslint/no-misused-promises': [
        'error',
        { checksVoidReturn: { attributes: false } },
      ],
    },
  },
  {
    // Node-context, non-type-checked tooling files.
    files: ['**/*.config.{js,ts}', 'playwright.config.ts'],
    languageOptions: { globals: { ...globals.node } },
    extends: [tseslint.configs.disableTypeChecked],
  },
  {
    // Tests and E2E specs run under Node + vitest/playwright globals, and
    // deliberately poke at the untyped wasm/Playwright boundaries — the
    // `no-unsafe-*` family is pure noise there.
    files: ['test/**/*', 'e2e/**/*'],
    languageOptions: { globals: { ...globals.node } },
    rules: {
      '@typescript-eslint/no-unsafe-assignment': 'off',
      '@typescript-eslint/no-unsafe-member-access': 'off',
      '@typescript-eslint/no-unsafe-call': 'off',
      '@typescript-eslint/no-unsafe-argument': 'off',
      '@typescript-eslint/no-unsafe-return': 'off',
      '@typescript-eslint/no-explicit-any': 'off',
      // Casts on `any` wasm values can't be proven necessary by the linter.
      '@typescript-eslint/no-unnecessary-type-assertion': 'off',
    },
  },
  {
    // The Vite entry module bootstraps the app (side effects), so the
    // react-refresh "only export components" rule doesn't apply.
    files: ['src/index.tsx'],
    rules: { 'react-refresh/only-export-components': 'off' },
  },
  // Keep ESLint clear of formatting concerns — Prettier owns those.
  prettier
);
