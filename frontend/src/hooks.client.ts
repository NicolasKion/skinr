// The engine is the slowest thing a visit waits for, so it starts with the
// app, before the page hydrates. The stage (#lib/components/stage.svelte)
// joins this start when it mounts; a failure is left for it to meet and show.
void import('#lib/viewer/engine.js').then(({ getEngine }) => getEngine()).catch(() => {});
