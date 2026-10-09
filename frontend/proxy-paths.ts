// The shared-origin routing contract, mirrored by deploy/Caddyfile.dev in
// docker: keep both in sync when a prefix changes.

// Paths Axum owns outright: every method proxies to the API.
export const axumPrefixes = ['/api', '/cdn'];
