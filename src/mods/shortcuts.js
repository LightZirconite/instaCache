// The phone app's gestures, and a few keys, on the web page.
//
//   double-click a picture or a video   like it, as a double tap does
//   L   like what is in view            M   sound on or off
//   C   comments                        S   save
//
// Space is left alone: on Reels it is Instagram's own way to the next reel.
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

function press(media, labels) {
    var button = control(postOf(media), labels);
    if (button)
        button.click();
    return !!button;
}

function like(media, x, y) {
    heart(x, y);
    // After Instagram has had its own say on the same gesture: if it liked
    // the post itself, "Like" is gone and nothing is pressed twice.
    setTimeout(function () { press(media, LABELS.like); }, 150);
}

document.addEventListener("dblclick", function (event) {
    var media = mediaAt(event.clientX, event.clientY);
    if (media && postOf(media))
        like(media, event.clientX, event.clientY);
});

document.addEventListener("keydown", function (event) {
    if (event.ctrlKey || event.altKey || event.metaKey || event.repeat || typing())
        return;
    var media = mediaInView();
    if (!media)
        return;
    var handled = false;
    switch (event.key) {
    case "l":
    case "L":
        var r = media.getBoundingClientRect();
        like(media, r.left + r.width / 2, r.top + r.height / 2);
        handled = true;
        break;
    case "m":
    case "M":
        handled = press(media, LABELS.audioMuted.concat(LABELS.audioPlaying));
        if (!handled && media instanceof HTMLVideoElement) {
            media.muted = !media.muted;
            handled = true;
        }
        break;
    case "c":
    case "C":
        handled = press(media, LABELS.comment);
        break;
    case "s":
    case "S":
        handled = press(media, LABELS.save);
        break;
    }
    if (handled)
        event.preventDefault();
});
