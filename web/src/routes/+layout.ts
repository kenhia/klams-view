// Static SPA: no SSR, no prerender beyond the shell — the axum server
// serves build/index.html as the fallback for every route.
export const ssr = false;
export const prerender = false;
