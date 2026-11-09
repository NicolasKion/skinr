// The render worker: draws ships on request, framed like a thumbnail, for the
// API (src/thumbnails.rs keeps every ship's thumbnails current, src/og draws
// liveries for the Open Graph cards). Internal only: the API reaches it at
// RENDERER_URL, browsers never do. Run from frontend/:
//
//   node scripts/render-server.ts
//
//   GET  /render?dna=<SOF DNA>&size=<px>&theme=dark|light         -> PNG
//   GET  /render/skinr/<design id>?ship=<type id>&size=&theme=    -> PNG
//   POST /thumbnail {dna, size?} or {design, ship, size?}
//                                   -> {pngs: {dark, light}, used, missing}
//   GET  /health
//
// A PNG is transparent. A thumbnail answers both themes as base64 PNGs, with
// every res:/ path the ship used (the page preloads them) and those it could
// not read. A SKINR design is read from the API and turned into DNA plus a
// SOF pattern by the site's own skinr.ts.
//
// The studio (scripts/render/studio.ts) lives in child processes
// (scripts/render/worker.ts), WORKERS of them, each warm between requests and
// taking one shot at a time. The runtime keeps everything it has loaded, so a
// child is replaced after MAX_RENDERS shots, or sooner when it grows past
// MAX_RSS_MB.

import { spawn, type ChildProcess } from 'node:child_process';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { createInterface } from 'node:readline';
import { designLook, DESIGN_ID } from './render/skinr-look.ts';
import { THEMES, type Theme } from './render/studio.ts';
import type { ShotAnswer, ShotRequest } from './render/worker.ts';

/** Where SKINR designs and ships are read from. */
const API = process.env.API ?? 'http://127.0.0.1:3100';
/** The port this server listens on. */
const PORT = Number(process.env.PORT ?? 3200);
/** Shots per worker process before a fresh one takes over. */
const MAX_RENDERS = Number(process.env.MAX_RENDERS ?? 50);
/** A worker using more memory than this, in MB, is replaced after its shot. */
const MAX_RSS_MB = Number(process.env.MAX_RSS_MB ?? 3072);
/** How long one shot may take before its worker is killed, in ms (lavapipe draws on the CPU). */
const SHOT_TIMEOUT_MS = Number(process.env.SHOT_TIMEOUT_MS ?? 180_000);
/** How long a worker may take to open its studio, in ms (a cold cache downloads the SOF). */
const STARTUP_TIMEOUT_MS = Number(process.env.STARTUP_TIMEOUT_MS ?? 300_000);
/**
 * Worker processes drawing at once. Each holds a studio (a few hundred MB to
 * a few GB as it loads ships), so this is bounded by memory and cores.
 */
const WORKERS = Math.max(1, Number(process.env.WORKERS ?? 1));
/** Requests waiting beyond this many are turned away. */
const MAX_QUEUE = Number(process.env.MAX_QUEUE ?? 32);
/** The picture size when a request names none, and the range it may ask for, in px. */
const DEFAULT_SIZE = 512;
const MIN_SIZE = 64;
const MAX_SIZE = Number(process.env.MAX_SIZE ?? 1024);
/** The longest DNA taken: real ones are a few hundred characters. */
const MAX_DNA_LENGTH = 2048;
/** The largest request body taken, in bytes (a thumbnail request is a DNA). */
const MAX_BODY_BYTES = 16 * 1024;

const WORKER = new URL('./render/worker.ts', import.meta.url).pathname;

interface Worker {
  process: ChildProcess;
  port: number;
  shots: number;
}

/** Starts a worker and waits until its studio is open. */
function startWorker(): Promise<Worker> {
  // stdin is held open and never written: the worker exits when it closes,
  // so it never outlives this process.
  const child = spawn(process.execPath, [WORKER], { stdio: ['pipe', 'pipe', 'inherit'] });
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => child.kill('SIGKILL'), STARTUP_TIMEOUT_MS);
    let ready = false;
    // Every line after the port is passed through.
    createInterface({ input: child.stdout! }).on('line', (line) => {
      const match = /^PORT (\d+)$/.exec(line);
      if (ready || !match) {
        console.log(line);
        return;
      }
      ready = true;
      clearTimeout(timeout);
      console.log(`worker ${child.pid} ready on ${match[1]}`);
      resolve({ process: child, port: Number(match[1]), shots: 0 });
    });
    child.on('exit', (code, signal) => {
      clearTimeout(timeout);
      if (!ready) reject(new Error(`the worker exited before it was ready (${signal ?? code})`));
    });
  });
}

/**
 * One place for a worker: the worker taking its shots, started on first use
 * and replaced as it ages. Each has its own studio, so slots draw at once;
 * a worker's shot is mostly one busy thread, so a few of them use the CPU
 * (or GPU) a single one leaves idle.
 */
interface Slot {
  worker: Promise<Worker> | null;
}

function startIn(slot: Slot) {
  const started = startWorker();
  slot.worker = started;
  started.catch((error: Error) => {
    console.error(`worker failed to start: ${error.message}`);
    if (slot.worker === started) slot.worker = null;
  });
}

function retire(slot: Slot, old: Worker, why: string) {
  console.log(`worker ${old.process.pid}: ${why}, starting a fresh one`);
  old.process.kill('SIGKILL');
  // The next one starts now, so the next request finds it warm.
  startIn(slot);
}

class ShotError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

/** Takes one shot on the worker in `slot`. */
async function shoot(slot: Slot, shot: ShotRequest): Promise<ShotAnswer> {
  if (!slot.worker) startIn(slot);
  let current: Worker;
  try {
    current = await slot.worker!;
  } catch (error) {
    slot.worker = null;
    throw new ShotError(503, `the studio did not open: ${(error as Error).message}`);
  }
  let response: Response;
  try {
    response = await fetch(`http://127.0.0.1:${current.port}/shot`, {
      method: 'POST',
      body: JSON.stringify(shot),
      signal: AbortSignal.timeout(SHOT_TIMEOUT_MS),
    });
  } catch (error) {
    const timedOut = (error as Error).name === 'TimeoutError';
    retire(slot, current, timedOut ? 'timed out' : 'stopped answering');
    throw new ShotError(504, `the shot failed: ${(error as Error).message}`);
  }
  current.shots += 1;
  const rss = Number(response.headers.get('x-rss') ?? 0) / 1024 / 1024;
  const body = await response.text();
  if (current.shots >= MAX_RENDERS) retire(slot, current, `${current.shots} shots`);
  else if (rss > MAX_RSS_MB) retire(slot, current, `${Math.round(rss)} MB`);
  if (!response.ok) throw new ShotError(422, body);
  return JSON.parse(body) as ShotAnswer;
}

const slots: Slot[] = Array.from({ length: WORKERS }, () => ({ worker: null }));
/** Slots without a shot, and shots waiting for one, first come first served. */
const free: Slot[] = [...slots];
const waiting: ((slot: Slot) => void)[] = [];

/** Runs `job` on the next free slot; each slot takes one shot at a time. */
async function enqueue<T>(job: (slot: Slot) => Promise<T>): Promise<T> {
  let slot = free.pop();
  if (!slot) {
    if (waiting.length >= MAX_QUEUE) throw new ShotError(503, 'too many renders waiting');
    slot = await new Promise<Slot>((resolve) => waiting.push(resolve));
  }
  try {
    return await job(slot);
  } finally {
    const next = waiting.shift();
    if (next) next(slot);
    else free.push(slot);
  }
}

/** A picture size from a request, or a message saying what is wrong. */
function sizeOf(value: unknown): number | string {
  const size = Number(value ?? DEFAULT_SIZE);
  return Number.isInteger(size) && size >= MIN_SIZE && size <= MAX_SIZE
    ? size
    : `size must be a whole number from ${MIN_SIZE} to ${MAX_SIZE}`;
}

/** The size and theme a picture request asks for, or a message saying what is wrong. */
function picture(params: URLSearchParams): { size: number; theme: Theme } | string {
  const size = sizeOf(params.get('size') ?? undefined);
  if (typeof size === 'string') return size;
  const theme = (params.get('theme') ?? 'dark') as Theme;
  if (!THEMES.includes(theme)) return `theme must be one of ${THEMES.join(', ')}`;
  return { size, theme };
}

const validDna = (dna: unknown): dna is string =>
  typeof dna === 'string' && dna.trim() !== '' && dna.length <= MAX_DNA_LENGTH;

/** A request's body, or null when it is larger than MAX_BODY_BYTES. */
async function readBody(request: IncomingMessage): Promise<string | null> {
  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of request) {
    length += (chunk as Buffer).length;
    if (length > MAX_BODY_BYTES) return null;
    chunks.push(chunk as Buffer);
  }
  return Buffer.concat(chunks).toString();
}

/** SKINR design `id` on ship `ship` as the SOF builds it, or what is wrong. */
async function design(id: unknown, ship: unknown) {
  const designId = String(id).toLowerCase();
  const shipId = Number(ship);
  if (!DESIGN_ID.test(designId)) return 'not a design id';
  if (!Number.isInteger(shipId) || shipId <= 0) return 'ship must be a type id';
  const look = await designLook(API, designId, shipId).catch(() => null);
  return look ?? 'No such design on that ship.';
}

/** What a request asks for: a shot and how to answer it, or an error to send. */
type Plan =
  | { shot: ShotRequest; answer: 'png' | 'thumbnail' }
  | { status: number; message: string };

async function plan(request: IncomingMessage, url: URL): Promise<Plan> {
  if (request.method === 'POST' && url.pathname === '/thumbnail') {
    const text = await readBody(request);
    if (text === null) return { status: 413, message: 'request too large' };
    let wanted: { dna?: unknown; design?: unknown; ship?: unknown; size?: unknown };
    try {
      wanted = JSON.parse(text);
    } catch {
      return { status: 400, message: 'not JSON' };
    }
    const size = sizeOf(wanted.size);
    if (typeof size === 'string') return { status: 400, message: size };
    const themes = [...THEMES];
    if (wanted.design !== undefined) {
      const look = await design(wanted.design, wanted.ship);
      if (typeof look === 'string') return { status: 404, message: look };
      return { shot: { dna: look.dna, pattern: look.pattern, size, themes }, answer: 'thumbnail' };
    }
    if (!validDna(wanted.dna)) return { status: 400, message: 'dna is missing or too long' };
    return { shot: { dna: wanted.dna.trim(), size, themes }, answer: 'thumbnail' };
  }
  if (request.method !== 'GET') return { status: 405, message: 'Only GET, or POST /thumbnail.' };
  const wanted = picture(url.searchParams);
  if (typeof wanted === 'string') return { status: 400, message: wanted };
  const { size, theme } = wanted;
  if (url.pathname === '/render') {
    const dna = url.searchParams.get('dna');
    if (!validDna(dna)) return { status: 400, message: 'dna is missing or too long' };
    return { shot: { dna: dna.trim(), size, themes: [theme] }, answer: 'png' };
  }
  if (url.pathname.startsWith('/render/skinr/')) {
    const look = await design(
      url.pathname.slice('/render/skinr/'.length),
      url.searchParams.get('ship'),
    );
    if (typeof look === 'string') return { status: 404, message: look };
    return {
      shot: { dna: look.dna, pattern: look.pattern, size, themes: [theme] },
      answer: 'png',
    };
  }
  return { status: 404, message: 'Not found.' };
}

async function handle(request: IncomingMessage, response: ServerResponse) {
  const url = new URL(request.url ?? '/', 'http://renderer');
  if (request.method === 'GET' && url.pathname === '/health') {
    response.end('ok');
    return;
  }
  const planned = await plan(request, url);
  if ('status' in planned) {
    response.writeHead(planned.status).end(planned.message);
    return;
  }
  const { shot, answer } = planned;
  let taken: ShotAnswer;
  try {
    taken = await enqueue((slot) => shoot(slot, shot));
  } catch (error) {
    const status = error instanceof ShotError ? error.status : 500;
    response.writeHead(status).end((error as Error).message);
    return;
  }
  if (answer === 'png') {
    response
      .writeHead(200, { 'content-type': 'image/png', 'cache-control': 'no-store' })
      .end(Buffer.from(taken.pngs[shot.themes[0]]!, 'base64'));
  } else {
    const { pngs, used, missing } = taken;
    response
      .writeHead(200, { 'content-type': 'application/json', 'cache-control': 'no-store' })
      .end(JSON.stringify({ pngs, used, missing }));
  }
}

// Every worker opens its studio before the first request needs it.
for (const slot of slots) startIn(slot);
createServer((request, response) => {
  handle(request, response).catch((error: Error) => {
    console.error(`request failed: ${error.message}`);
    if (!response.headersSent) response.writeHead(500);
    response.end();
  });
}).listen(PORT, '0.0.0.0', () => console.log(`Listening on http://0.0.0.0:${PORT}/`));
