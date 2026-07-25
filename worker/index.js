export default {
  async fetch() {
    const headers = new Headers({ "Content-Type": "text/plain; charset=utf-8" });
    setCrossOriginIsolationHeaders(headers);
    return new Response("Not Found", { status: 404, headers });
  },
};

function setCrossOriginIsolationHeaders(headers) {
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Cross-Origin-Embedder-Policy", "require-corp");
  headers.set("Cross-Origin-Resource-Policy", "same-origin");
}
