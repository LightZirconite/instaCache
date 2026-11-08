// Instagram's colours instead of black, and the colours of what is on screen.
//
// Instagram paints its dark theme from `--ig-*` variables written "R, G, B".
// This writes them from one tint: a tint per kind of page — the feed, Reels,
// messages, Explore, a profile — and, with `options.ambient`, the colour of
// the picture or video in view mixed in, the way a video player's ambient
// mode lights the room. A change of tint is eased over a third of a second.
// And the tint drifts, slowly, towards a neighbouring colour and back, so the
// page is never quite one flat colour — see `drift`.
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
// Each page's tint, and the colour it drifts to and back. Deep enough to sit
// behind white text — none is lighter than a tenth of white — and coloured
// enough to read as Instagram's purple, magenta and blue rather than black.
var PAGES = {
    home: [30, 12, 44],
    reels: [26, 8, 36],
    direct: [16, 16, 48],
    explore: [38, 10, 36],
    profile: [32, 14, 46],
    login: [30, 12, 44]
};
var DRIFTS = {
    home: [44, 10, 38],
    reels: [36, 8, 30],
    direct: [26, 12, 50],
    explore: [46, 12, 26],
    profile: [24, 14, 52],
    login: [44, 10, 38]
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

function write(tint) {
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
        if (step >= 4) {
            clearInterval(easing);
            easing = null;
        }
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
    var saturation = Math.min(0.9, (max - min) / max * 1.6);
    var grey = 24;
    return rgb.map(function (c) {
        var hue = c / max * 58;
        return grey * (1 - saturation) + hue * saturation;
    });
}

// What the tint was last worked out from, so the same media is read once.
var lastKind = null, lastMedia = null, lastColour = null;

// Where the drift is: 0 at the page's tint, 1 at its neighbour, over
// `DRIFT_PERIOD_MS` there and back.
var DRIFT_PERIOD_MS = 90000;
var DRIFT_EVERY_MS = 6000;
function phase() {
    return 0.5 - 0.5 * Math.cos(2 * Math.PI * (Date.now() % DRIFT_PERIOD_MS) / DRIFT_PERIOD_MS);
}

function target(page) {
    var base = PAGES[page], other = DRIFTS[page], at = options.ambient ? phase() : 0;
    var tint = base.map(function (c, i) { return c + (other[i] - c) * at; });
    if (!lastColour)
        return tint;
    var weight = page === "reels" ? 0.85 : 0.7;
    return tint.map(function (c, i) { return c + (lastColour[i] - c) * weight; });
}

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
    lastColour = colour || null;
    ease(target(page));
}

// The drift, with `options.ambient`: the colours that move. Every step
// restyles the whole page like any other change — measured at 55 to 90 ms
// for ten thousand elements on a slow machine with no GPU — so a step is
// taken only where nobody can feel it: the window focused, nothing
// scrolling, no video playing in view, nobody typing, not in Direct, no ease
// under way. On Reels, where a video always plays, the tint holds still,
// which is the point: restyling under a playing reel is what made the next
// one stutter. Each step is a few units out of 255, too small to see as one.
function playing() {
    var media = mediaInView();
    return media instanceof HTMLVideoElement && !media.paused;
}
setInterval(function () {
    var html = document.documentElement;
    if (!options.ambient || !html || !html.classList.contains(MARK)
        || lastKind === null || lastKind === "login" || lastKind === "direct")
        return;
    if (document.visibilityState !== "visible" || !document.hasFocus() || scrolling
        || easing !== null || typing() || playing())
        return;
    write(target(lastKind));
}, DRIFT_EVERY_MS);

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
var resting = null, scrolling = false;
document.addEventListener("scroll", function () {
    scrolling = true;
    clearTimeout(resting);
    resting = setTimeout(function () {
        scrolling = false;
        refresh();
    }, 300);
}, true);
