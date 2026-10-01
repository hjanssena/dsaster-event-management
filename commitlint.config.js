module.exports = {
  extends: ['@commitlint/config-conventional'],
  parserPreset: {
    parserOpts: {
      // Matches: feat(VE05T1): message
      headerPattern: /^(\w+)(?:\(([^)]+)\))?: (.+)$/,
      headerCorrespondence: ['type', 'scope', 'subject'],
    },
  },
  rules: {
    // Require scope to be present
    'scope-empty': [2, 'never'],
    // Require scope to follow alphanumeric uppercase format (e.g., VE05T1)
    'scope-case': [0], // disabled standard casing to allow custom ticket formats
    // Scope must match your specific pattern
    'scope-pattern': [2, 'always'],
    // Standard allowed types
    'type-enum': [
      2,
      'always',
      [
        'feat',
        'fix',
        'docs',
        'style',
        'refactor',
        'perf',
        'test',
        'chore',
        'ci',
        'build',
      ],
    ],
    // Basic formatting rules
    'type-case': [2, 'always', 'lower-case'],
    'type-empty': [2, 'never'],
    'subject-empty': [2, 'never'],
    'subject-full-stop': [2, 'never', '.'],
  },
  plugins: [
    {
      rules: {
        'scope-pattern': ({ scope }) => {
          // Matches alphanumeric ticket format (e.g., VE05T1)
          const regex = /^[A-Z0-9_-]+$/;
          const isValid = scope && regex.test(scope);
          return [
            isValid,
            `Scope must match format: uppercase alphanumeric ticket ID (e.g. VE05T1). Received: "${scope}"`,
          ];
        },
      },
    },
  ],
};
