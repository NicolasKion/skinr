// The shared-origin routing contract, mirrored by deploy/Caddyfile in
// docker and production: keep both in sync when a prefix changes.

// Paths Axum owns outright: every method proxies to the API.
export const axumPrefixes = ['/api', '/cdn'];
