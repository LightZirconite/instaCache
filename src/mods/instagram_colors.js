// Instagram's colours instead of black, and the colours of what is on screen.
//
// Instagram paints its dark theme from `--ig-*` variables written "R, G, B".
// This writes them from one tint: a tint per kind of page — the feed, Reels,
// messages, Explore, a profile — and, with `options.ambient`, the colour of
// the picture or video in view mixed in, the way a video player's ambient
// mode lights the room. A change of tint is eased over a third of a second.
//
// Every change restyles the whole page, so it is rare: the colour is read
// once per picture or video, when one comes into view and has a frame to
// read — never on a timer, and never in the middle of a scroll. Reading it
// every two seconds while a reel played made the next reel stutter in.
//
// Only while the page is dark. The class Instagram uses for that changes
// from build to build — `_aa4d`, `__fb-dark-mode` and `__ig-dark-mode` have
// all been seen — so the colour it paints is what is read, and `<html>` is
// marked with a class of ours, which instagram_colors.css keys on.
var MARK = "instacache-dark";
var PAGES = {
    home: [18, 12, 26],
    reels: [16, 8, 22],
    direct: [12, 12, 30],
    explore: [24, 10, 24],
    profile: [20, 13, 28],
    login: [18, 12, 26]
};
var theirs = null;
var palette = null;
var shown = PAGES.home;
var easing = null;

function kind() {
    var path = location.pathname;
    // Signed out, the home page is the login page too.
    if (path.indexOf("/accounts/login") === 0 || document.getElementById("login_form"))
        return "login";
    if (path === "/")
        return "home";
    if (/^\/reels?\//.test(path))
        return "reels";
    if (path.indexOf("/direct/") === 0)
        return "direct";
    if (path.indexOf("/explore/") === 0)
        return "explore";
    if (/^\/[^/]+\/?$/.test(path))
        return "profile";
    return "home";
}

function lift(rgb, by) {
    return rgb.map(function (c) { return Math.min(255, Math.round(c + by)); }).join(", ");
}

function hex(rgb) {
    return "#" + rgb.map(function (c) {
        return ("0" + Math.round(c).toString(16)).slice(-2);
    }).join("");
}

// No channel of a tint is ever above this, nor below 0, nor anything but a
// number: whatever reaches the page is a dark colour. A tint out of range
// is the only way this mod could paint the page a bright colour, and the
// owner's page once came up bright red; the cause was not found, so the
// bound stands guard.
var DARKEST = 0, LIGHTEST = 48;

function write(tint) {
    if (!tint.every(function (c) { return typeof c === "number" && isFinite(c); }))
        return;
    tint = tint.map(function (c) { return Math.min(LIGHTEST, Math.max(DARKEST, c)); });
    shown = tint;
    if (!palette) {
        palette = document.createElement("style");
        palette.id = "instacache-palette";
        (document.head || document.documentElement).appendChild(palette);
    }
    var raised = tint.map(function (c) { return c + 13; });
    palette.textContent = "html." + MARK + " {" +
        "--ig-primary-background: " + lift(tint, 0) + " !important;" +
        "--ig-secondary-background: " + lift(tint, 20) + " !important;" +
        "--ig-elevated-background: " + lift(tint, 13) + " !important;" +
        "--ig-highlight-background: " + lift(tint, 28) + " !important;" +
        "--ig-banner-background: " + lift(tint, 20) + " !important;" +
        "--ig-secondary-elevated-background: " + lift(tint, 28) + " !important;" +
        "--ig-elevated-highlight-background: " + lift(tint, 34) + " !important;" +
        "--ig-bubble-background: " + lift(tint, 13) + " !important;" +
        "--ig-incoming-message-bubble: " + lift(tint, 28) + " !important;" +
        "--card-background: " + hex(raised) + " !important;" +
        "--surface-background: " + hex(raised) + " !important;" +
        "--popover-card-background: " + hex(raised) + " !important;" +
        "--nav-bar-background: " + hex(raised) + " !important;" +
        "--messenger-card-background: " + hex(raised) + " !important;" +
        "--toast-background: " + hex(tint.map(function (c) { return c + 20; })) + " !important; }";
}

// Four steps rather than every frame: each one restyles the whole page.
function ease(to) {
    if (to.every(function (c, i) { return Math.abs(c - shown[i]) < 4; }))
        return;
    clearInterval(easing);
    var from = shown, step = 0;
    easing = setInterval(function () {
        step++;
        var t = step / 4;
        write(from.map(function (c, i) { return c + (to[i] - c) * t; }));
        if (step >= 4)
            clearInterval(easing);
    }, 80);
}

// The colour of a picture or video, dark enough to sit behind white text:
// its average hue and some of its saturation, at a fixed low lightness.
var sample = document.createElement("canvas");
sample.width = sample.height = 8;
function colourOf(media) {
    if (media instanceof HTMLVideoElement ? media.readyState < 2 : !media.complete)
        return null;
    try {
        var context = sample.getContext("2d", { willReadFrequently: true });
        context.clearRect(0, 0, 8, 8);
        context.drawImage(media, 0, 0, 8, 8);
        var data = context.getImageData(0, 0, 8, 8).data, r = 0, g = 0, b = 0, n = 0;
        // Weighted by opacity: a video with no picture draws nothing, and
        // nothing is not black.
        for (var i = 0; i < data.length; i += 4) {
            var a = data[i + 3] / 255;
            r += data[i] * a;
            g += data[i + 1] * a;
            b += data[i + 2] * a;
            n += a;
        }
        return n < 1 ? null : darken([r / n, g / n, b / n]);
    } catch (e) {
        // A picture served without CORS cannot be read; it keeps the tint.
        return null;
    }
}

function darken(rgb) {
    var max = Math.max.apply(null, rgb) || 1;
    var min = Math.min.apply(null, rgb);
    var saturation = Math.min(0.75, (max - min) / max * 1.3);
    var grey = 22;
    return rgb.map(function (c) {
        var hue = c / max * 42;
        return grey * (1 - saturation) + hue * saturation;
    });
}

// What the tint was last worked out from, so the same media is read once.
var lastKind = null, lastMedia = null;

function refresh() {
    var html = document.documentElement;
    if (!html || !html.classList.contains(MARK))
        return;
    var page = kind();
    // What the stylesheet keys the login page's look on.
    if (html.getAttribute("data-instacache-page") !== page)
        html.setAttribute("data-instacache-page", page);
    var media = options.ambient && page !== "login" ? mediaInView() : null;
    if (page === lastKind && media === lastMedia)
        return;
    var colour = media && colourOf(media);
    if (media && !colour && media instanceof HTMLVideoElement && media.readyState < 2) {
        // No frame yet: try again once it has one, and only then.
        media.addEventListener("loadeddata", refresh, { once: true });
        return;
    }
    lastKind = page;
    lastMedia = media;
    var base = PAGES[page];
    var weight = page === "reels" ? 0.8 : 0.6;
    ease(colour ? base.map(function (c, i) { return c + (colour[i] - c) * weight; }) : base);
}

function check(force) {
    var html = document.documentElement;
    if (!html)
        return;
    var classes = html.className.replace(MARK, "").trim();
    // Only our own class changed: nothing to read again.
    if (!force && classes === theirs)
        return;
    theirs = classes;
    // Instagram's colour, not ours.
    html.classList.remove(MARK);
    var rgb = getComputedStyle(html).getPropertyValue("--ig-primary-background")
        .split(",").slice(0, 3).map(parseFloat);
    var dark = rgb.length === 3 && rgb.every(isFinite)
        && 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2] < 64;
    if (dark) {
        html.classList.add(MARK);
        refresh();
    }
}

function watch() {
    var html = document.documentElement;
    if (!html)
        return false;
    new MutationObserver(function () { check(false); })
        .observe(html, { attributes: true, attributeFilter: ["class"] });
    check(true);
    return true;
}

if (!watch()) {
    new MutationObserver(function (_, observer) {
        if (watch())
            observer.disconnect();
    }).observe(document, { childList: true });
}
// The variables arrive with Instagram's stylesheets, which can be after the
// theme class does, and the desktop's theme can change under a running page.
document.addEventListener("DOMContentLoaded", function () { check(true); });
window.addEventListener("load", function () { check(true); });
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", function () {
    check(true);
});
onPageChange(refresh);
// Once a scroll has come to rest, not during it.
var resting = null;
document.addEventListener("scroll", function () {
    clearTimeout(resting);
    resting = setTimeout(refresh, 300);
}, true);
