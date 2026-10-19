import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
  AXUM_URL: {
    description: 'Where server-side fetches to /api go.',
    schema: (value) => value ?? 'http://127.0.0.1:3100',
  },
});
