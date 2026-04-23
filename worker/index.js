export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    let key = url.pathname.replace(/^\/+/, "");
    if (key === "" || key.endsWith("/")) {
      key = `${key}index.html`;
    }

    const object = await env.BUCKET.get(key);
    if (!object) {
      return new Response("Not Found", { status: 404 });
    }

    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);

    if (key.endsWith(".html")) {
      headers.set("Cache-Control", "no-cache");
    } else if (key.startsWith("assets/")) {
      // Vite hashes these filenames; safe to cache forever.
      headers.set("Cache-Control", "public, max-age=31536000, immutable");
    } else {
      headers.set("Cache-Control", "public, max-age=3600");
    }

    return new Response(object.body, { headers });
  },
};
