// One warm studio behind a local port: scripts/render-server.ts starts this
// process, hands it one shot at a time and replaces it once it has rendered
// enough (the runtime keeps everything it ever loaded).
//
//   POST /shot {dna, size, themes, pattern?: {name, record}} -> ShotAnswer
//
// Prints `PORT <n>` on stdout once it listens; everything else goes to stderr.
// Exits when its stdin closes, which is when the server that started it goes.

import { createServer } from 'node:http';
import { THEMES, openStudio, type Theme } from './studio.ts';

/** Where CDN files are kept between runs. */
const CACHE = process.env.CACHE ?? '../storage/cdn-cache';
/** The size the studio opens at; requests may ask for others. */
const SIZE = Number(process.env.SIZE ?? 512);
/** The studio look per theme, as JSON: {"dark": {...}, "light": {...}} (lighting.ts). */
const LOOKS_OVERRIDE = process.env.LOOKS;

export interface ShotRequest {
  dna: string;
  size: number;
  themes: Theme[];
  pattern?: { name: string; record: object } | null;
}

/** A shot taken: a base64 PNG per theme, and the files the ship used and missed. */
export interface ShotAnswer {
  pngs: Partial<Record<Theme, string>>;
  used: string[];
  missing: string[];
  ms: number;
}

const studio = await openStudio({
  cache: CACHE,
  size: SIZE,
  looks: LOOKS_OVERRIDE ? JSON.parse(LOOKS_OVERRIDE) : undefined,
});

async function take(shot: ShotRequest): Promise<ShotAnswer> {
  const started = performance.now();
  if (shot.pattern) studio.registerPattern(shot.pattern.name, shot.pattern.record);
  const { pngs, used, missing } = await studio.render(shot.dna, shot.themes, shot.size);
  const ms = Math.round(performance.now() - started);
  const note = missing.length ? `, ${missing.length} missing` : '';
  console.error(`${shot.dna} at ${shot.size} px: ${ms} ms${note}`);
  const encoded: ShotAnswer['pngs'] = {};
  for (const theme of shot.themes) encoded[theme] = Buffer.from(pngs[theme]!).toString('base64');
  return { pngs: encoded, used, missing, ms };
}

const server = createServer(async (request, response) => {
  // The server reads this to retire a worker that grew too big.
  response.setHeader('x-rss', String(process.memoryUsage().rss));
  if (request.method !== 'POST' || request.url !== '/shot') {
    response.writeHead(404).end('Not found.');
    return;
  }
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(chunk as Buffer);
  const shot: ShotRequest = JSON.parse(Buffer.concat(chunks).toString());
  if (!shot.themes?.length || !shot.themes.every((theme) => THEMES.includes(theme))) {
    response.writeHead(400).end('Unknown theme.');
    return;
  }
  try {
    const answer = await take(shot);
    response.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(answer));
  } catch (error) {
    const message = (error as Error).message;
    console.error(`${shot.dna}: failed: ${message}`);
    response.writeHead(422).end(message);
  }
});
server.listen(0, '127.0.0.1', () => {
  const address = server.address();
  if (address && typeof address === 'object') console.log(`PORT ${address.port}`);
});

// The server never writes to stdin; it closes when the server exits.
process.stdin.on('end', () => process.exit(0));
process.stdin.resume();
