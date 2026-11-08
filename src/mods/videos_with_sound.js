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
// The volume is set once per video. Instagram has no control for it, so `+`
// and `-` set it here, and the last one chosen is remembered in the
// profile's own storage for the page — every video after it, and every
// session after this one, starts there. Until one is chosen,
// `options.volume` from the config is used. The mod steps aside for the rest of the page's life as
// soon as the user mutes a video with their own hand — a video that had its
// sound. Instagram creating the next video muted, a moment after a click on
// "Reels", is not the user muting anything; reading it as such switched the
// mod off for the whole session, and the owner's videos arrived silent.
var userMuted = false;
// Videos that have been heard: only muting one of these is the user's doing.
var heard = new WeakSet();
// How many times a video has been unmuted for the user. A page that mutes it
// straight back would otherwise be fought for ever, one event at a time.
var forced = new WeakMap();
var MAX_FORCED = 3;
var tuned = new WeakSet();

// Kept in the page's localStorage, which is the profile's: it lives as long
// as the session does, needs no way back into Rust, and takes effect in the
// very page that set it.
var VOLUME_KEY = "instacache:video-volume";
function storedVolume() {
    try {
        var v = parseFloat(localStorage.getItem(VOLUME_KEY));
        return v >= 0 && v <= 1 ? v : null;
    } catch (e) {
        return null;
    }
}
var volume = storedVolume();
if (volume === null)
    volume = options.volume;

function remember(v) {
    volume = Math.round(Math.min(1, Math.max(0, v)) * 100) / 100;
    try {
        localStorage.setItem(VOLUME_KEY, String(volume));
    } catch (e) {
        // A full or refused storage keeps the volume for this page only.
    }
}

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
    var speaker = speakerOf(media);
    if (saysMuted(speaker))
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
        media.volume = volume;
    }
    unmute(media);
}, true);

document.addEventListener("volumechange", function (event) {
    var media = event.target;
    if (!(media instanceof HTMLMediaElement) || isCall(media))
        return;
    if (!media.muted) {
        heard.add(media);
        if (byUser()) {
            userMuted = false;
            // A volume the user set by some other means than `+` and `-`
            // — the page's own control, should Instagram add one.
            if (tuned.has(media) && Math.abs(media.volume - volume) > 0.005)
                remember(media.volume);
        }
        return;
    }
    if (byUser() && heard.has(media))
        userMuted = true;
    else if (!media.paused)
        unmute(media);
}, true);

// `+` and `-`: the volume of every video, in steps of a tenth, remembered.
// Shown for a moment at the top of the window, since nothing else shows it.
var gauge = null, gaugeTimer = null;
addStyle("instacache-mod-volume",
    ".instacache-volume { position: fixed; z-index: 2147483647; top: 18px; left: 50%;" +
    "  pointer-events: none; padding: 8px 16px; border-radius: 18px;" +
    "  background: rgba(0, 0, 0, 0.72); color: #fff; font: 600 14px/18px system-ui, sans-serif;" +
    "  opacity: 0; transform: translate(-50%, -6px);" +
    "  transition: opacity 160ms ease-out, transform 160ms ease-out; }" +
    ".instacache-volume.shown { opacity: 1; transform: translate(-50%, 0); }" +
    "@media (prefers-reduced-motion: reduce) { .instacache-volume { transition: none; } }");

function showVolume() {
    if (!gauge) {
        gauge = document.createElement("div");
        gauge.className = "instacache-volume";
        gauge.setAttribute("aria-live", "polite");
        document.body.appendChild(gauge);
    }
    gauge.textContent = (volume === 0 ? "\uD83D\uDD07 " : "\uD83D\uDD0A ")
        + Math.round(volume * 100) + " %";
    // Shown on the next frame, so the transition runs from the hidden state.
    requestAnimationFrame(function () { gauge.classList.add("shown"); });
    clearTimeout(gaugeTimer);
    gaugeTimer = setTimeout(function () { gauge.classList.remove("shown"); }, 1100);
}

window.addEventListener("keydown", function (event) {
    if (event.ctrlKey || event.altKey || event.metaKey || typing())
        return;
    var step = event.key === "+" || event.key === "=" ? 0.1 : event.key === "-" ? -0.1 : 0;
    if (step === 0)
        return;
    remember(volume + step);
    var all = document.querySelectorAll("video, audio");
    for (var i = 0; i < all.length; i++) {
        if (isCall(all[i]))
            continue;
        tuned.add(all[i]);
        all[i].volume = volume;
    }
    // Louder means heard: a muted video in view gets its sound, through
    // Instagram's own switch, as `M` would.
    var media = mediaInView();
    if (step > 0 && media instanceof HTMLVideoElement && media.muted) {
        userMuted = false;
        var speaker = speakerOf(media);
        if (saysMuted(speaker))
            speaker.click();
        else
            media.muted = false;
    }
    showVolume();
    event.preventDefault();
    event.stopImmediatePropagation();
}, true);
