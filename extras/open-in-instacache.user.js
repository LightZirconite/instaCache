// ==UserScript==
// @name         Open in instaCache
// @namespace    https://github.com/LightZirconite/instaCache
// @version      1.0.0
// @description  On instagram.com in your browser, a button to open the same page in instaCache.
// @match        https://www.instagram.com/*
// @grant        none
// @run-at       document-idle
// ==/UserScript==
//
// For Violentmonkey, Tampermonkey or Greasemonkey. The button hands the page
// to instaCache through the `instacache:` scheme that its menu entry
// declares; the browser asks once whether to allow that. "Not now" hides the
// button for the rest of the visit in that tab.

(function () {
    "use strict";
    var KEY = "instacache-open-dismissed";
    try {
        if (sessionStorage.getItem(KEY))
            return;
    } catch (e) {
        // Storage refused: show the button anyway.
    }

    var bar = document.createElement("div");
    bar.style.cssText = [
        "position:fixed", "right:16px", "bottom:16px", "z-index:2147483647",
        "display:flex", "gap:8px", "align-items:center", "padding:8px 10px",
        "border-radius:14px", "background:rgba(20,12,28,0.92)", "color:#fff",
        "font:600 13px system-ui,sans-serif", "box-shadow:0 8px 24px rgba(0,0,0,0.35)"
    ].join(";");

    var open = document.createElement("button");
    open.textContent = "Open in instaCache";
    open.style.cssText = [
        "border:0", "border-radius:10px", "padding:7px 12px", "cursor:pointer",
        "color:#fff", "font:inherit",
        "background:linear-gradient(135deg,#9B30B8,#DD2A7B 55%,#FF9A3C)"
    ].join(";");
    open.onclick = function () {
        location.href = "instacache:" + location.href;
    };

    var later = document.createElement("button");
    later.textContent = "Not now";
    later.style.cssText = "border:0;background:none;color:#ccc;cursor:pointer;font:inherit";
    later.onclick = function () {
        try {
            sessionStorage.setItem(KEY, "1");
        } catch (e) {
            // Nothing to remember it in; it goes for this page only.
        }
        bar.remove();
    };

    bar.appendChild(open);
    bar.appendChild(later);
    document.body.appendChild(bar);
})();
