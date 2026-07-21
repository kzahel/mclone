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

    const versioned = url.searchParams.has("v");
    if (key.endsWith(".html")) {
      headers.set("Cache-Control", "no-cache, max-age=0, must-revalidate");
    } else if (versioned && isVersionedRuntimeAsset(key)) {
      headers.set("Cache-Control", "public, max-age=31536000, immutable");
    } else if (isHashedViteAsset(key)) {
      headers.set("Cache-Control", "public, max-age=31536000, immutable");
    } else if (key.startsWith("animals/catalog/")) {
      headers.set("Cache-Control", "no-cache, max-age=0, must-revalidate");
    } else if (key.endsWith(".js") || key.endsWith(".wasm") || key.endsWith(".zip.json")) {
      headers.set("Cache-Control", "no-cache, max-age=0, must-revalidate");
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

function isVersionedRuntimeAsset(key) {
  return key.endsWith(".js") || key.endsWith(".wasm") || key.endsWith(".zip");
}

function isHashedViteAsset(key) {
  return key.startsWith("assets/") || key.includes("/assets/");
}
