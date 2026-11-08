// The phone app's gestures, and a few keys, on the web page.
//
//   double-click a picture or a video   like it, as a double tap does
//   right-click a picture or a video    repost it
//   Space, Space                        on Reels, like the reel and stay on
//                                       it; one Space still goes to the next
//   Enter, Enter                        like what is in view
//   L   like      R   repost      M   sound on or off
//   C   comments  S   save        O   turn the video sideways, full screen
//   click a story                       pause it, or play it again
//   mouse wheel over an open story      the next story, or the one before
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

var HEART = "M12 21.35 10.55 20C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 " +
    "7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 " +
    "8.5c0 3.78-3.4 6.86-8.55 11.54z";
// Two arrows chasing each other round: the usual sign for a repost.
var REPOST = "M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z";

// What a gesture did, drawn where it was made, the way the phone shows it.
function pop(x, y, shape, colour) {
    if (calm.matches)
        return;
    var ns = "http://www.w3.org/2000/svg";
    var svg = document.createElementNS(ns, "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("class", "instacache-heart");
    svg.style.left = x + "px";
    svg.style.top = y + "px";
    var path = document.createElementNS(ns, "path");
    path.setAttribute("fill", colour);
    path.setAttribute("d", shape);
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
    pop(x, y, HEART, "#ff3040");
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
    like(media, event.clientX, event.clientY);
    keepPlaying(media);
});

// A right-click on a picture or a video reposts it. The engine's own menu is
// off unless `context_menu` asks for it, and then the right-click is left to
// it. The event reaches the page either way: Chromium hands it to the page
// first and shows its menu only when the page lets it. Nothing is kept from
// the page when there is no repost button to press.
document.addEventListener("contextmenu", function (event) {
    if (options.contextMenu)
        return;
    var media = mediaAt(event.clientX, event.clientY);
    if (!media || !postOf(media) || !control(postOf(media), LABELS.repost))
        return;
    event.preventDefault();
    if (paced() && press(media, LABELS.repost))
        pop(event.clientX, event.clientY, REPOST, "#ffffff");
}, true);

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

// The move to the next reel, eased over `GLIDE_MS`. Not `behavior: "smooth"`:
// Qt WebEngine switches Chromium's scroll animator off, so a smooth scroll
// asked for by the page lands in one frame — measured, 0 to 796 pixels
// between two frames — and the reels jumped. Snapping is suspended while it
// moves, or the scroller would be pulled back to the reel it is leaving at
// every frame, and put back when it arrives.
var GLIDE_MS = 380;
var gliding = null;

function glide(scroller, by) {
    if (gliding)
        gliding.stop();
    var from = scroller.scrollTop;
    var to = Math.max(0, Math.min(from + by, scroller.scrollHeight - scroller.clientHeight));
    if (calm.matches || to === from) {
        scroller.scrollTop = to;
        return;
    }
    var snap = scroller.style.getPropertyValue("scroll-snap-type");
    scroller.style.setProperty("scroll-snap-type", "none");
    var start = null, frame = 0;
    var move = {
        stop: function () {
            cancelAnimationFrame(frame);
            scroller.style.setProperty("scroll-snap-type", snap);
            if (!snap)
                scroller.style.removeProperty("scroll-snap-type");
            gliding = null;
        }
    };
    function step(now) {
        if (start === null)
            start = now;
        var t = Math.min(1, (now - start) / GLIDE_MS);
        scroller.scrollTop = from + (to - from) * (1 - Math.pow(1 - t, 3));
        if (t < 1)
            frame = requestAnimationFrame(step);
        else
            move.stop();
    }
    gliding = move;
    frame = requestAnimationFrame(step);
}
// A wheel or a press takes over from a glide rather than fighting it.
["wheel", "pointerdown"].forEach(function (type) {
    window.addEventListener(type, function () {
        if (gliding)
            gliding.stop();
    }, { capture: true, passive: true });
});

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
                glide(scroller, scroller.clientHeight);
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
    case "o":
    case "O":
        handled = media instanceof HTMLVideoElement && turn(media);
        break;
    }
    if (handled) {
        event.preventDefault();
        event.stopImmediatePropagation();
    }
}, true);

// `O` turns the video in view a quarter turn, full screen, so a clip filmed
// sideways fills the screen the way it was shot; `O` again turns it the
// other way, a third time — or Escape, or a click — puts it back.
//
// The picture is a copy: the video's own frames, through `captureStream()`,
// shown by a video of this mod's own in a layer over the page. Turning
// Instagram's element itself is not possible — its containers clip it, and
// the browser refuses any transform on an element in full screen. The copy
// costs no second decoding, and Instagram's video goes on playing, and
// being heard, underneath; the copy is silent. Nothing is sent anywhere.
addStyle("instacache-mod-shortcuts-turn",
    ".instacache-turn { position: fixed; inset: 0; z-index: 2147483647; background: #000;" +
    "  cursor: zoom-out; animation: instacache-turn-in 220ms ease-out; }" +
    ".instacache-turn video { position: absolute; left: 50%; top: 50%;" +
    "  width: 100vh; height: 100vw; object-fit: contain;" +
    "  transition: transform 260ms cubic-bezier(0.2, 0.8, 0.2, 1); }" +
    "@keyframes instacache-turn-in { from { opacity: 0; } to { opacity: 1; } }" +
    "@media (prefers-reduced-motion: reduce) {" +
    "  .instacache-turn { animation: none; } .instacache-turn video { transition: none; } }");

var turned = null;

function closeTurn() {
    if (!turned)
        return;
    var layer = turned.layer;
    turned = null;
    layer.remove();
    if (document.fullscreenElement === layer)
        document.exitFullscreen().catch(function () {});
}

function turn(media) {
    if (turned) {
        if (turned.angle === 90) {
            turned.angle = -90;
            turned.copy.style.transform = "translate(-50%, -50%) rotate(-90deg)";
        } else
            closeTurn();
        return true;
    }
    var stream;
    try {
        stream = media.captureStream();
    } catch (e) {
        // A video from another origin cannot be copied; it stays as it is.
        return false;
    }
    var layer = document.createElement("div");
    layer.className = "instacache-turn";
    var copy = document.createElement("video");
    copy.muted = true;
    copy.autoplay = true;
    copy.playsInline = true;
    copy.srcObject = stream;
    copy.style.transform = "translate(-50%, -50%) rotate(90deg)";
    layer.appendChild(copy);
    layer.addEventListener("click", closeTurn);
    document.body.appendChild(layer);
    turned = { layer: layer, copy: copy, source: media, angle: 90, wasFull: false };
    copy.play().catch(function () {});
    // Full screen when the engine allows it; a layer over the window when not.
    if (layer.requestFullscreen)
        layer.requestFullscreen().catch(function () {});
    return true;
}

// Leaving full screen by any way — Escape, F11 — puts the video back.
document.addEventListener("fullscreenchange", function () {
    if (turned && document.fullscreenElement !== turned.layer && turned.wasFull)
        closeTurn();
    else if (turned && document.fullscreenElement === turned.layer)
        turned.wasFull = true;
});
window.addEventListener("keydown", function (event) {
    if (turned && event.key === "Escape") {
        closeTurn();
        event.preventDefault();
        event.stopImmediatePropagation();
    }
}, true);
// Another page, and the copy has nothing left to show.
onPageChange(function () {
    if (turned && !document.contains(turned.source))
        closeTurn();
});

// The wheel over an open story goes to the next story, or back to the one
// before: one story for one turn of the wheel, however many events a
// touchpad sends for it. The step presses Instagram's own arrow button —
// "Suivant", "Précédente", captured on the owner's open story — and sends
// the arrow key Instagram answers on a story where the button is not found,
// as in a language whose labels are not captured yet. The wheel has to rest
// for `QUIET_MS` before it can move another story.
var QUIET_MS = 250;
var wheelSum = 0, wheelLocked = false, wheelQuiet = null;

function step(forward) {
    var button = control(document, forward ? LABELS.storyNext : LABELS.storyPrevious);
    if (button)
        button.click();
    else
        arrow(forward ? "ArrowRight" : "ArrowLeft");
}

function arrow(key) {
    var event = new KeyboardEvent("keydown", { key: key, code: key, bubbles: true, cancelable: true });
    var code = key === "ArrowRight" ? 39 : 37;
    Object.defineProperty(event, "keyCode", { get: function () { return code; } });
    Object.defineProperty(event, "which", { get: function () { return code; } });
    (document.activeElement || document.body).dispatchEvent(event);
}

document.addEventListener("wheel", function (event) {
    if (!onStory() || event.ctrlKey)
        return;
    event.preventDefault();
    clearTimeout(wheelQuiet);
    wheelQuiet = setTimeout(function () {
        wheelLocked = false;
        wheelSum = 0;
    }, QUIET_MS);
    if (wheelLocked)
        return;
    var dy = Math.abs(event.deltaY) >= Math.abs(event.deltaX) ? event.deltaY : event.deltaX;
    wheelSum += dy * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? innerHeight : 1);
    if (Math.abs(wheelSum) < 40)
        return;
    wheelLocked = true;
    step(wheelSum > 0);
}, { passive: false });

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
    if (onStory() || event.ctrlKey || event.shiftKey || Math.abs(event.deltaX) >= Math.abs(event.deltaY))
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
