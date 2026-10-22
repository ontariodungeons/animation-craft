// HyperFrames 0.8.181 caption autosave omits the server's optimistic-write headers.
// Reuse the version actually read by this editor; conflicts stay blocked, never retried.
(() => {
  const fetch = globalThis.fetch.bind(globalThis), versions = new Map();
  globalThis.fetch = async (input, init) => {
    const url = new URL(typeof input === "string" || input instanceof URL ? input : input.url, location.href);
    if (url.origin !== location.origin || !/^\/api\/projects\/[^/]+\/files\/caption-overrides\.json$/.test(url.pathname)) return fetch(input, init);
    const method = (init?.method || (typeof input === "object" ? input.method : "GET") || "GET").toUpperCase();
    const headers = new Headers(init?.headers || (typeof input === "object" ? input.headers : undefined));
    const known = versions.get(url.href);
    if (method === "PUT" && known && !headers.has("If-Match") && !headers.has("If-None-Match")) {
      headers.set(known.missing ? "If-None-Match" : "If-Match", known.missing ? "*" : known.version);
    }
    const response = await fetch(input, {...init, headers});
    if (method === "GET" && response.status === 404) versions.set(url.href, {missing: true});
    else if ((method === "GET" || method === "PUT") && response.ok) {
      const version = response.headers.get("ETag");
      if (version) versions.set(url.href, {version});
    }
    return response;
  };
})();
