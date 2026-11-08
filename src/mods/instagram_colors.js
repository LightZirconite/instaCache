// Marks <html> with `instacache-dark` while Instagram shows its dark theme,
// which is what instagram_colors.css keys on.
//
// The class Instagram itself uses for it changes from build to build —
// `_aa4d`, `__fb-dark-mode` and `__ig-dark-mode` have all been seen — so it is
// not named here. The colour is the stable signal: the page is dark when the
// background it paints from is dark.
var MARK = "instacache-dark";
// Instagram's own classes the last time the colour was read.
var theirs = null;

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
    if (dark)
        html.classList.add(MARK);
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
