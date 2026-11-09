<script lang="ts">
  // The site bar: one glass capsule floating at the top of every page. On the
  // left the logo, the site's three areas (Ships, the Paragon Hub, your own
  // skins) and the page's own title; on the right the page's sections, with
  // a pill sliding under the one in view, and its reset button; then the
  // theme switch and the character (or a login). The bar never changes shape
  // while scrolling.
  import { ChevronRight, Moon, RotateCcw, Sun } from '@lucide/svelte';
  import { page } from '$app/state';
  import Logo from '#lib/components/logo.svelte';
  import { characterPortrait } from '#lib/eve.js';
  import { nav } from '#lib/nav.svelte.js';
  import { theme } from '#lib/theme.svelte.js';
  import type { Me } from '#lib/types.js';

  const me = $derived(page.data.me as Me | undefined);
  /** Back to this page after logging in. */
  const loginHref = $derived(
    `${me?.login ?? '/eve/login'}?return=${encodeURIComponent(page.url.pathname + page.url.search)}`,
  );

  /** The site's areas, each current on its own routes. */
  const areas = [
    {
      href: '/',
      label: 'Ships',
      current: (route: string) => route === '/' || route.startsWith('/ships'),
    },
    {
      href: '/skinr',
      label: 'Paragon Hub',
      /** What a phone's bar has room for. */
      short: 'Hub',
      current: (route: string) => route.startsWith('/skinr'),
    },
    { href: '/me', label: 'My skins', current: (route: string) => route === '/me' },
  ];
  const route = $derived(page.route.id ?? '');

  let links = $state<Record<string, HTMLAnchorElement>>({});
  const pill = $derived.by(() => {
    const link = nav.active ? links[nav.active] : null;
    return link ? { left: link.offsetLeft, width: link.offsetWidth } : null;
  });

  function jump(event: MouseEvent, id: string) {
    const target = document.getElementById(id);
    if (!target) return;
    event.preventDefault();
    const top = target.getBoundingClientRect().top + window.scrollY;
    window.scrollTo({ top, behavior: 'smooth' });
  }
</script>

<header class="pointer-events-none fixed inset-x-0 top-3 z-40 px-3 lg:px-6">
  <nav
    class="bar pointer-events-auto relative flex h-12 items-center gap-1 rounded-full pr-1.5 pl-3 sm:gap-2 sm:pr-2 sm:pl-4"
    aria-label="Site"
  >
    <a href="/" class="flex items-center gap-2 text-foreground/90 hover:text-foreground">
      <Logo class="size-[1.35rem]" />
      <!-- A phone's bar keeps the logo and gives the name's room to the areas. -->
      <span class="hidden text-sm font-semibold tracking-tight sm:inline"
        >skinr<span class="font-normal text-foreground/45">.space</span></span
      >
    </a>

    <div class="ml-1 flex items-center gap-0.5 sm:ml-3">
      {#each areas as area (area.href)}
        {@const current = area.current(route)}
        <a
          href={area.href}
          class="area px-2 py-1.5 text-[0.8rem] whitespace-nowrap min-[22rem]:px-2.5 sm:px-3"
          class:current
          aria-current={current ? 'page' : undefined}
        >
          {#if area.short}
            <span class="sm:hidden">{area.short}</span>
            <span class="hidden sm:inline">{area.label}</span>
          {:else}
            {area.label}
          {/if}
        </a>
      {/each}
    </div>

    {#if nav.title}
      <!-- On a phone the page's own big title says it. -->
      <ChevronRight class="hidden size-3.5 shrink-0 text-foreground/30 sm:block" />
      <span class="hidden truncate text-sm font-medium text-foreground sm:inline">{nav.title}</span>
    {/if}

    {#if nav.center}
      <!-- The page's docked controls, sliding in when its own row scrolls away. -->
      <div
        class="center mx-4 hidden min-w-0 flex-1 justify-center lg:flex"
        class:shown={nav.centerShown}
        inert={!nav.centerShown}
      >
        {@render nav.center()}
      </div>
    {/if}

    <div class="ml-auto flex items-center gap-1">
      {#if nav.sections.length}
        <div class="relative hidden items-center lg:flex">
          {#if pill}
            <span class="pill" style:left="{pill.left}px" style:width="{pill.width}px"></span>
          {/if}
          {#each nav.sections as section (section.id)}
            <a
              bind:this={links[section.id]}
              href="#{section.id}"
              class="section relative px-3.5 py-1.5 text-[0.8rem]"
              class:active={nav.active === section.id}
              aria-current={nav.active === section.id ? 'location' : undefined}
              onclick={(event) => jump(event, section.id)}
            >
              {section.label}
            </a>
          {/each}
        </div>
        <span class="mx-2 hidden h-5 w-px bg-foreground/10 lg:block"></span>
      {/if}

      {#if nav.reset}
        <!-- A phone's bar has no room for it; the drag is the way to turn there. -->
        <span class="hidden sm:contents">
          <button
            type="button"
            class="icon-button"
            title="Reset view"
            aria-label="Reset view"
            onclick={() => nav.reset?.()}
          >
            <RotateCcw class="size-4" />
          </button>
        </span>
      {/if}
      <a
        href="https://github.com/carbonenginejs"
        class="hidden px-3 py-1.5 text-[0.8rem] text-foreground/55 hover:text-foreground md:block"
      >
        CarbonEngineJS
      </a>
      <button
        type="button"
        class="icon-button"
        title={theme.current === 'dark' ? 'Light mode' : 'Dark mode'}
        aria-label={theme.current === 'dark' ? 'Light mode' : 'Dark mode'}
        onclick={() => theme.toggle()}
      >
        {#if theme.current === 'dark'}
          <Sun class="size-4" />
        {:else}
          <Moon class="size-4" />
        {/if}
      </button>
      {#if me?.character}
        <a
          href="/me"
          class="avatar ml-1"
          class:current={page.route.id === '/me'}
          title="Your SKINRs ({me.character.name})"
          aria-label="Your SKINRs"
        >
          <img src={characterPortrait(me.character.id, 64)} alt="" class="size-full" />
        </a>
      {:else if me?.login}
        <!-- Axum answers this one, not the page router. -->
        <a
          href={loginHref}
          data-sveltekit-reload
          class="ml-0.5 rounded-full bg-foreground px-2.5 py-1.5 text-[0.8rem] font-medium whitespace-nowrap text-background transition-transform hover:scale-[1.03] min-[22rem]:px-3 sm:ml-1 sm:px-4"
        >
          Log in
        </a>
      {/if}
    </div>
  </nav>
</header>

<style>
  /* A soft scrim behind the floating bar: content scrolling past fades out
     at the top edge instead of peeking out above the capsule. */
  header::before {
    content: '';
    position: absolute;
    inset: -0.75rem 0 auto;
    height: 5.5rem;
    background: linear-gradient(var(--background) 15%, transparent);
    pointer-events: none;
  }
  .bar {
    /* Stays put through page transitions (layout.css). */
    view-transition-name: site-bar;
    background: var(--glass-strong);
    backdrop-filter: saturate(160%) blur(28px);
    box-shadow:
      inset 0 0 0 1px var(--glass-edge),
      0 12px 40px -12px var(--shadow);
  }
  .center {
    opacity: 0;
    translate: 0 0.6rem;
    transition:
      opacity 0.35s var(--ease-apple),
      translate 0.5s var(--ease-apple);
  }
  .center.shown {
    opacity: 1;
    translate: 0 0;
  }
  .area {
    border-radius: 999px;
    color: color-mix(in oklab, var(--foreground) 55%, transparent);
    transition:
      color 0.3s var(--ease-apple),
      background 0.3s var(--ease-apple);
  }
  .area:hover {
    color: var(--foreground);
  }
  .area.current {
    color: var(--foreground);
    background: color-mix(in oklab, var(--foreground) 9%, transparent);
  }
  .section {
    color: color-mix(in oklab, var(--foreground) 55%, transparent);
    transition: color 0.3s var(--ease-apple);
  }
  .section:hover,
  .section.active {
    color: var(--foreground);
  }
  /* Slides under whichever section is in view. */
  .pill {
    position: absolute;
    top: 50%;
    height: 2rem;
    translate: 0 -50%;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 9%, transparent);
    transition:
      left 0.45s var(--ease-apple),
      width 0.45s var(--ease-apple);
  }
  .icon-button {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 999px;
    color: color-mix(in oklab, var(--foreground) 70%, transparent);
    transition:
      color 0.2s,
      background 0.2s;
  }
  .icon-button:hover {
    color: var(--foreground);
    background: color-mix(in oklab, var(--foreground) 8%, transparent);
  }
  .avatar {
    display: block;
    width: 2rem;
    height: 2rem;
    overflow: hidden;
    border-radius: 999px;
    box-shadow: inset 0 0 0 1px var(--glass-edge);
    transition: box-shadow 0.3s var(--ease-apple);
  }
  .avatar:hover,
  .avatar.current {
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--foreground) 35%, transparent);
  }
</style>
