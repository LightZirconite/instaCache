// Videos with sound.
//
// Instagram starts every video muted and keeps the switch to itself: a click
// on the page does not unmute it, only its own speaker button does, once per
// session. This starts each video with sound at `options.volume`, and steps
// aside for the rest of the page's life as soon as the user mutes one with
// their own hand.
//
// Runs at document creation, before Instagram's own code. It only listens;
// nothing of the page is replaced.
var userMuted = false;
// How many times a video has been unmuted for the user. A page that mutes it
// straight back would otherwise be fought for ever, one event at a time.
var forced = new WeakMap();
var MAX_FORCED = 3;

function isCall(media) {
    return typeof MediaStream !== "undefined" && media.srcObject instanceof MediaStream;
}

function byUser() {
    return !!(navigator.userActivation && navigator.userActivation.isActive);
}

function unmute(media) {
    if (userMuted || isCall(media) || !media.muted)
        return;
    var count = forced.get(media) || 0;
    if (count >= MAX_FORCED)
        return;
    forced.set(media, count + 1);
    media.volume = options.volume;
    media.muted = false;
}

// Media events do not bubble; listening in the capture phase on the document
// still hears every one of them.
document.addEventListener("play", function (event) {
    if (event.target instanceof HTMLMediaElement)
        unmute(event.target);
}, true);

document.addEventListener("volumechange", function (event) {
    var media = event.target;
    if (!(media instanceof HTMLMediaElement) || isCall(media))
        return;
    if (media.muted) {
        if (byUser())
            userMuted = true;
        else
            unmute(media);
    } else if (byUser()) {
        userMuted = false;
    }
}, true);
