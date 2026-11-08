// What the mods share: Instagram's words for the controls they look for, and
// ways of finding things in a page whose class names change at every build.
//
// Every label here was read off instagram.com, in English and in French —
// "Also from Meta" from the owner's own window — not guessed. A control whose
// label is in no list is simply not found, and the mod that wanted it does
// nothing rather than click something else.
var LABELS = {
    audioMuted: ["Audio is muted", "Le son est coupé"],
    audioPlaying: ["Audio is playing", "L’audio est en cours de lecture"],
    like: ["Like", "J’aime"],
    comment: ["Comment", "Commenter"],
    save: ["Save", "Enregistrer"],
    alsoFromMeta: ["Also from Meta"]
};

// The clickable control under `root` whose icon carries one of `labels`.
function control(root, labels) {
    if (!root)
        return null;
    var icons = root.querySelectorAll("svg[aria-label]");
    for (var i = 0; i < icons.length; i++) {
        if (labels.indexOf(icons[i].getAttribute("aria-label")) >= 0)
            return icons[i].closest("[role=button], button, a");
    }
    return null;
}

// The post a picture or a video belongs to: the nearest ancestor that also
// holds a Comment button, which every post has whether or not it is liked.
function postOf(media) {
    for (var e = media, depth = 0; e && depth < 16; e = e.parentElement, depth++) {
        if (control(e, LABELS.comment))
            return e;
    }
    return null;
}

function isMedia(e) {
    if (!(e instanceof HTMLVideoElement || e instanceof HTMLImageElement))
        return false;
    var r = e.getBoundingClientRect();
    return r.width >= 200 && r.height >= 200;
}

// The picture or video being looked at: the largest one across the middle
// of the window.
function mediaInView() {
    var middle = innerHeight / 2, best = null, bestArea = 0;
    var all = document.querySelectorAll("video, img");
    for (var i = 0; i < all.length; i++) {
        var r = all[i].getBoundingClientRect();
        if (r.width < 200 || r.height < 200 || r.top > middle || r.bottom < middle)
            continue;
        var area = (Math.min(r.bottom, innerHeight) - Math.max(r.top, 0))
            * (Math.min(r.right, innerWidth) - Math.max(r.left, 0));
        if (area > bestArea) {
            best = all[i];
            bestArea = area;
        }
    }
    return best;
}

// The picture or video under a point, through whatever Instagram lays over
// it to catch clicks.
function mediaAt(x, y) {
    var stack = document.elementsFromPoint(x, y);
    for (var i = 0; i < stack.length; i++) {
        if (isMedia(stack[i]))
            return stack[i];
    }
    return null;
}

function typing() {
    var e = document.activeElement;
    return !!e && (e.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(e.tagName));
}

// `fn` runs after the page changed — Instagram navigates without loading —
// at most every 400 ms, whatever the page does in between.
var pageWatchers = [];
function onPageChange(fn) {
    pageWatchers.push(fn);
}
(function () {
    var timer = null;
    function run() {
        timer = null;
        for (var i = 0; i < pageWatchers.length; i++) {
            try {
                pageWatchers[i]();
            } catch (e) {
                console.error("instacache: mod failed:", e);
            }
        }
    }
    function schedule() {
        if (timer === null)
            timer = setTimeout(run, 400);
    }
    function start() {
        if (!document.body)
            return false;
        new MutationObserver(schedule).observe(document.body, { childList: true, subtree: true });
        schedule();
        return true;
    }
    if (!start())
        document.addEventListener("DOMContentLoaded", start);
})();
