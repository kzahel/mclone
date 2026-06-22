export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    let key = url.pathname.replace(/^\/+/, "");
    if (key === "" || key.endsWith("/")) {
      key = `${key}index.html`;
    }

    const object = await env.BUCKET.get(key);
    if (!object) {
      const headers = new Headers({ "Content-Type": "text/plain; charset=utf-8" });
      setCrossOriginIsolationHeaders(headers);
      return new Response("Not Found", { status: 404, headers });
    }

    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);
    setCrossOriginIsolationHeaders(headers);

    if (key.endsWith(".html") || key.endsWith(".zip.json")) {
      headers.set("Cache-Control", "no-cache");
    } else if (key.startsWith("assets/")) {
      // Vite hashes these filenames; safe to cache forever.
      headers.set("Cache-Control", "public, max-age=31536000, immutable");
    } else if (key.endsWith(".zip")) {
      headers.set("Cache-Control", "public, max-age=3600");
    } else {
      headers.set("Cache-Control", "public, max-age=3600");
    }

    return new Response(object.body, { headers });
  },
};

function setCrossOriginIsolationHeaders(headers) {
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Cross-Origin-Embedder-Policy", "require-corp");
  headers.set("Cross-Origin-Resource-Policy", "same-origin");
}
