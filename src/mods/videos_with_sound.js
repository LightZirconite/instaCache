// Videos with sound.
//
// Instagram starts every video muted and keeps one switch for all of them:
// its speaker button. Turning the sound on behind its back — `muted = false`
// on the element — played the first video with sound under an icon that
// still said muted, and the next video started muted again. So the switch
// that is pressed is Instagram's own, once: its icon is right, and every
// video after this one starts with sound. Where no speaker button is found,
// the element is unmuted directly, as before.
//
// The volume is set once per video, to `options.volume`; Instagram has no
// control for it. The mod steps aside for the rest of the page's life as
// soon as the user mutes a video with their own hand.
var userMuted = false;
// How many times a video has been unmuted for the user. A page that mutes it
// straight back would otherwise be fought for ever, one event at a time.
var forced = new WeakMap();
var MAX_FORCED = 3;
var tuned = new WeakSet();

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
    var speaker = control(postOf(media), LABELS.audioMuted);
    if (speaker)
        speaker.click();
    if (media.muted)
        media.muted = false;
}

// Media events do not bubble; listening in the capture phase on the document
// still hears every one of them.
document.addEventListener("play", function (event) {
    var media = event.target;
    if (!(media instanceof HTMLMediaElement) || isCall(media))
        return;
    if (!tuned.has(media)) {
        tuned.add(media);
        media.volume = options.volume;
    }
    unmute(media);
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
