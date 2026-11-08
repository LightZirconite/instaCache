// The phone app's gestures, and a few keys, on the web page.
//
//   double-click a picture or a video   like it, as a double tap does
//   Shift + double-click                repost it
//   Space, Space                        on Reels, like the reel and stay on
//                                       it; one Space still goes to the next
//   Enter, Enter                        like what is in view
//   L   like      R   repost      M   sound on or off
//   C   comments  S   save
//   click a story                       pause it, or play it again
//   mouse wheel over the stories row    scroll the row, which otherwise only
//                                       moves with an arrow that misses
//
// Every action presses Instagram's own button for it, found by its label
// (see page.js), so the page stays the one deciding. A like only ever likes:
// an unliked post is the one showing "Like", and a liked one is left alone,
// as on the phone. Keys do nothing while typing.
var calm = matchMedia("(prefers-reduced-motion: reduce)");

addStyle("instacache-mod-shortcuts-heart",
    "@keyframes instacache-heart {" +
    "  0% { opacity: 0; transform: translate(-50%, -50%) scale(0.4); }" +
    "  25% { opacity: 1; transform: translate(-50%, -50%) scale(1.15); }" +
    "  45% { transform: translate(-50%, -50%) scale(0.95); }" +
    "  70% { opacity: 1; transform: translate(-50%, -50%) scale(1); }" +
    "  100% { opacity: 0; transform: translate(-50%, -60%) scale(1); } }" +
    ".instacache-heart { position: fixed; z-index: 2147483647; pointer-events: none;" +
    "  width: 96px; height: 96px; animation: instacache-heart 800ms ease-out forwards;" +
    "  filter: drop-shadow(0 4px 14px rgba(0, 0, 0, 0.35)); }");

function heart(x, y) {
    if (calm.matches)
        return;
    var ns = "http://www.w3.org/2000/svg";
    var svg = document.createElementNS(ns, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("class", "instacache-heart");
    svg.style.left = x + "px";
    svg.style.top = y + "px";
    var path = document.createElementNS(ns, "path");
    path.setAttribute("fill", "#ff3040");
    path.setAttribute("d", "M12 21.35 10.55 20C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 " +
        "7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 " +
        "8.5c0 3.78-3.4 6.86-8.55 11.54z");
    svg.appendChild(path);
    document.body.appendChild(svg);
    setTimeout(function () { svg.remove(); }, 900);
}

// What reaches Instagram's servers — a like, a repost, a save — happens at
// most once every `PACE_MS`, whatever the keyboard does: one action for one
// gesture, at a person's pace, never a burst.
var PACE_MS = 800;
var lastAction = 0;
function paced() {
    var now = Date.now();
    if (now - lastAction < PACE_MS)
        return false;
    lastAction = now;
    return true;
}

function press(media, labels) {
    var button = control(postOf(media), labels);
    if (button)
        button.click();
    return !!button;
}

function like(media, x, y) {
    if (!paced())
        return;
    heart(x, y);
    // After Instagram has had its own say on the same gesture: if it liked
    // the post itself, "Like" is gone and nothing is pressed twice.
    setTimeout(function () { press(media, LABELS.like); }, 150);
}

function likeInView(media) {
    var r = media.getBoundingClientRect();
    like(media, r.left + r.width / 2, r.top + r.height / 2);
}

// A double-click is two clicks first, and Instagram pauses a reel on a
// click: the like came with a pause. So the state the video was in when the
// first click went down is kept, and put back once the like is done.
var beforeClicks = null;
document.addEventListener("pointerdown", function (event) {
    var media = mediaAt(event.clientX, event.clientY);
    if (!(media instanceof HTMLVideoElement))
        return;
    var now = Date.now();
    if (!beforeClicks || beforeClicks.media !== media || now - beforeClicks.at > 600)
        beforeClicks = { media: media, paused: media.paused, at: now };
}, true);

function keepPlaying(media) {
    var before = beforeClicks;
    if (!before || before.media !== media)
        return;
    setTimeout(function () {
        if (media.paused === before.paused)
            return;
        if (before.paused)
            media.pause();
        else
            media.play();
    }, 250);
}

document.addEventListener("dblclick", function (event) {
    var media = mediaAt(event.clientX, event.clientY);
    if (!media || !postOf(media))
        return;
    if (event.shiftKey) {
        if (paced())
            press(media, LABELS.repost);
    } else
        like(media, event.clientX, event.clientY);
    keepPlaying(media);
});

// A click on a story pauses it, or plays it again: Instagram only does that
// from its small button at the top. A click on one of the story's own
// controls — reply, like, share, the arrows — is theirs, not this.
document.addEventListener("click", function (event) {
    if (!onStory() || event.target.closest("a, button, input, textarea, [contenteditable]"))
        return;
    var own = event.target.closest("[role=button]");
    if (own && own.querySelector("svg[aria-label]"))
        return;
    if (!mediaAt(event.clientX, event.clientY))
        return;
    var toggle = control(document, LABELS.storyPause.concat(LABELS.storyPlay));
    if (toggle)
        toggle.click();
});

// The first key of a pair, and what was in view when it went down.
var DOUBLE_MS = 350;
var first = { key: null, at: 0, media: null };

function second(key, media) {
    var now = Date.now();
    var pair = first.key === key && now - first.at < DOUBLE_MS ? first.media : null;
    first = pair ? { key: null, at: 0, media: null } : { key: key, at: now, media: media };
    return pair;
}

// On Reels, one Space goes to the next reel and two like the one on screen,
// staying on it — as a double tap does. Telling them apart means holding the
// first Space for `SPACE_MS`: the reels scroll on their own when no second
// one comes. Letting the first Space through and liking on the second, as
// 2.10 did, liked the reel and skipped it as well.
var SPACE_MS = 250;
var pendingNext = null;

function onReels() {
    return /^\/reels?\//.test(location.pathname);
}

// What scrolls the reels: the nearest ancestor of the video that scrolls
// vertically, or the page itself.
function scrollerOf(media) {
    for (var e = media.parentElement; e && e !== document.documentElement; e = e.parentElement) {
        if (e.scrollHeight > e.clientHeight + 1) {
            var overflow = getComputedStyle(e).overflowY;
            if (overflow === "auto" || overflow === "scroll")
                return e;
        }
    }
    var page = document.scrollingElement;
    return page && page.scrollHeight > page.clientHeight + 1 ? page : null;
}

function focusIsFree() {
    var e = document.activeElement;
    return !e || e === document.body || !e.closest("a, button, [role=button]");
}

// In the capture phase on the window, which runs before any listener
// Instagram adds, so a key handled here can be kept from it.
window.addEventListener("keydown", function (event) {
    if (event.ctrlKey || event.altKey || event.metaKey || event.repeat || typing())
        return;
    var media = mediaInView();
    if (!media)
        return;
    var handled = false;
    var target = null;
    switch (event.key) {
    case " ":
        if (!onReels() || event.shiftKey)
            break;
        var scroller = scrollerOf(media);
        // Nothing found to scroll: Space stays Instagram's.
        if (!scroller)
            break;
        if (pendingNext !== null) {
            clearTimeout(pendingNext);
            pendingNext = null;
            likeInView(media);
        } else {
            pendingNext = setTimeout(function () {
                pendingNext = null;
                scroller.scrollBy({ top: scroller.clientHeight, behavior: "smooth" });
            }, SPACE_MS);
        }
        handled = true;
        break;
    case "Enter":
        if (!focusIsFree())
            break;
        target = second("Enter", media);
        if (target) {
            likeInView(target);
            handled = true;
        }
        break;
    case "l":
    case "L":
        likeInView(media);
        handled = true;
        break;
    case "r":
    case "R":
        handled = paced() && press(media, LABELS.repost);
        break;
    case "m":
    case "M":
        var speaker = speakerOf(media);
        if (speaker)
            speaker.click();
        else if (media instanceof HTMLVideoElement)
            media.muted = !media.muted;
        handled = !!speaker || media instanceof HTMLVideoElement;
        break;
    case "c":
    case "C":
        handled = press(media, LABELS.comment);
        break;
    case "s":
    case "S":
        // Saves, or takes it back out, as the phone's button does.
        handled = paced() && press(media, LABELS.save.concat(LABELS.saved));
        break;
    }
    if (handled) {
        event.preventDefault();
        event.stopImmediatePropagation();
    }
}, true);

// The wheel over a row that scrolls sideways — the stories at the top of the
// feed, a scroll container of its own, measured at 12 489 pixels of avatars
// in 630 — scrolls the row. Only a short row, so a large area that happens to
// overflow sideways never takes the page's scrolling, and only while the row
// can still move that way: at its end, the page scrolls as usual.
function sideways(start) {
    for (var e = start; e && e !== document.body; e = e.parentElement) {
        if (e.scrollWidth <= e.clientWidth + 1 || e.clientHeight > 260)
            continue;
        var overflow = getComputedStyle(e).overflowX;
        if (overflow === "auto" || overflow === "scroll")
            return e;
    }
    return null;
}

document.addEventListener("wheel", function (event) {
    if (event.ctrlKey || event.shiftKey || Math.abs(event.deltaX) >= Math.abs(event.deltaY))
        return;
    var row = sideways(event.target);
    if (!row)
        return;
    var step = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? row.clientWidth : 1);
    var room = step > 0 ? row.scrollWidth - row.clientWidth - row.scrollLeft : row.scrollLeft;
    if (room <= 0)
        return;
    row.scrollBy({ left: step * 1.5, behavior: "smooth" });
    event.preventDefault();
}, { passive: false });
