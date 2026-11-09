<script lang="ts">
  // A page's head for search engines and link previews: its title and
  // description, the canonical URL, Open Graph and Twitter tags pointing at
  // the API's card for it (`/og/…`, 1200x630, src/og), and structured data.
  import { page } from '$app/state';

  let {
    title,
    description,
    image = '/og/home.png',
    path = page.url.pathname,
    type = 'website',
    noindex = false,
    jsonLd = [],
  }: {
    /** The page's own title; the site's name is added. */
    title: string;
    /** One or two sentences, as a search result shows them. */
    description: string;
    /** The page's Open Graph card, a path on this site. */
    image?: string;
    /** The canonical path (the page's own by default). */
    path?: string;
    type?: 'website' | 'article' | 'profile';
    /** Keeps the page out of search results (a character's own page). */
    noindex?: boolean;
    /** schema.org objects for the page, as JSON-LD. */
    jsonLd?: object[];
  } = $props();

  /** The site's name, after every title and in previews. */
  const SITE = 'skinr.space';
  /** The cards' size, which unfurlers read before fetching them. */
  const CARD_WIDTH = 1200;
  const CARD_HEIGHT = 630;

  const origin = $derived((page.data.origin as string | undefined) ?? page.url.origin);
  const url = $derived(`${origin}${path}`);
  const imageUrl = $derived(`${origin}${image}`);
  const fullTitle = $derived(title === SITE ? title : `${title} · ${SITE}`);
  /** JSON-LD inside a script tag: `<` escaped so the data can never close it. */
  const structured = $derived(
    jsonLd.map(
      (data) =>
        `<script type="application/ld+json">${JSON.stringify({ '@context': 'https://schema.org', ...data }).replaceAll('<', '\\u003c')}</` +
        'script>',
    ),
  );
</script>

<svelte:head>
  <title>{fullTitle}</title>
  <meta name="description" content={description} />
  <link rel="canonical" href={url} />
  {#if noindex}
    <meta name="robots" content="noindex" />
  {/if}
  <meta property="og:site_name" content={SITE} />
  <meta property="og:type" content={type} />
  <meta property="og:title" content={title} />
  <meta property="og:description" content={description} />
  <meta property="og:url" content={url} />
  <meta property="og:image" content={imageUrl} />
  <meta property="og:image:width" content={String(CARD_WIDTH)} />
  <meta property="og:image:height" content={String(CARD_HEIGHT)} />
  <meta property="og:image:alt" content={title} />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content={title} />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={imageUrl} />
  {#each structured as script (script)}
    <!-- Structured data the page builds itself, not text from the game. -->
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    {@html script}
  {/each}
</svelte:head>
