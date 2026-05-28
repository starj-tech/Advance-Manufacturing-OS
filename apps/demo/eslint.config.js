import config from '@aether/eslint-config/react.js';

export default [
  ...config,
  {
    files: ['src/**/*.{ts,tsx}'],
    rules: {
      // The desktop app is the integration point — allow some flexibility
      // on console output for the dev-mode login flow until PR #2.
      'no-console': 'off',
    },
  },
];
