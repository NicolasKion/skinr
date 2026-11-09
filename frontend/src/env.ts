import { defineEnvVars } from '@sveltejs/kit/env';

export const variables = defineEnvVars({
  AXUM_URL: {
    description: 'Where server-side fetches to /api go.',
    schema: (value) => value ?? 'http://127.0.0.1:3100',
  },
  PUBLIC_ORIGIN: {
    description:
      'The site as visitors reach it (https://skinr.space), for canonical links and Open Graph tags; the request origin when unset.',
    schema: (value) => value || undefined,
  },
  DEBUG_TOOLS: {
    description:
      "Set to 1 to show the debug tools (drawing a livery's thumbnails); the API needs it too. Off in production.",
    schema: (value) => value === '1',
  },
});
