// Share-link helpers used by both the playground and the explorer.
//
// Encodes the editor source into `#code=<tag><base64url-payload>` so the
// link is fully self-contained — no backend, no storage. `d1` payloads are
// raw-deflate compressed (CompressionStream); `b1` are raw UTF-8 bytes
// for browsers without the streams API. Decoding accepts either.

(function () {
    function b64urlEncode(bytes) {
        let bin = '';
        for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
        return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
    }

    function b64urlDecode(str) {
        str = str.replace(/-/g, '+').replace(/_/g, '/');
        while (str.length % 4) str += '=';
        const bin = atob(str);
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        return bytes;
    }

    async function encodeShare(src) {
        const enc = new TextEncoder().encode(src);
        if (typeof CompressionStream !== 'undefined') {
            try {
                const cs = new CompressionStream('deflate-raw');
                const stream = new Blob([enc]).stream().pipeThrough(cs);
                const buf = new Uint8Array(await new Response(stream).arrayBuffer());
                if (buf.length < enc.length) return 'd1' + b64urlEncode(buf);
            } catch (_) { /* fall through to plain */ }
        }
        return 'b1' + b64urlEncode(enc);
    }

    async function decodeShare(payload) {
        if (payload.length < 3) throw new Error('payload too short');
        const tag = payload.slice(0, 2);
        const body = payload.slice(2);
        const bytes = b64urlDecode(body);
        if (tag === 'd1') {
            if (typeof DecompressionStream === 'undefined') {
                throw new Error('compressed link, but browser lacks DecompressionStream');
            }
            const ds = new DecompressionStream('deflate-raw');
            const stream = new Blob([bytes]).stream().pipeThrough(ds);
            const out = await new Response(stream).arrayBuffer();
            return new TextDecoder().decode(out);
        }
        if (tag === 'b1') return new TextDecoder().decode(bytes);
        throw new Error('unknown share format: ' + tag);
    }

    // Reads `#code=...` from the current URL and returns the decoded source,
    // or null if absent. Logs to console on decode failure rather than
    // throwing — a bad fragment shouldn't break page load.
    async function readSharedSource() {
        const m = /(?:^|[#&])code=([A-Za-z0-9_\-]+)/.exec(location.hash || '');
        if (!m) return null;
        try {
            return await decodeShare(m[1]);
        } catch (e) {
            console.warn('Could not decode shared source:', e);
            return null;
        }
    }

    async function buildShareURL(src) {
        const payload = await encodeShare(src);
        const base = location.href.split('#')[0];
        return base + '#code=' + payload;
    }

    async function copyShareLink(src) {
        const url = await buildShareURL(src);
        if (navigator.clipboard && navigator.clipboard.writeText) {
            await navigator.clipboard.writeText(url);
        } else {
            const ta = document.createElement('textarea');
            ta.value = url;
            ta.style.position = 'fixed';
            ta.style.opacity = '0';
            document.body.appendChild(ta);
            ta.select();
            try { document.execCommand('copy'); } finally { document.body.removeChild(ta); }
        }
        return url;
    }

    function showToast(msg, kind) {
        let host = document.getElementById('omg-toast');
        if (!host) {
            host = document.createElement('div');
            host.id = 'omg-toast';
            host.className = 'toast';
            document.body.appendChild(host);
        }
        host.textContent = msg;
        host.classList.remove('is-error');
        if (kind === 'error') host.classList.add('is-error');
        host.classList.add('is-visible');
        clearTimeout(showToast._t);
        showToast._t = setTimeout(() => host.classList.remove('is-visible'), 2200);
    }

    window.OMGShare = {
        encodeShare,
        decodeShare,
        readSharedSource,
        buildShareURL,
        copyShareLink,
        showToast,
    };
})();
