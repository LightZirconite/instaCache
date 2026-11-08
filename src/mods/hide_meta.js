// Hides "Also from Meta" in Instagram's side bar: a menu of Meta's other
// apps, which is advertising rather than navigation. Found by its icon's
// label, or failing that by its text, and hidden — never removed, so
// Instagram's own code finds the element where it left it.
//
// Every copy is hidden, at every change of the page, not just the first one
// found. The owner's logged-in page, captured in French, held one: a link
// (`a[role=link]`) around the labelled icon, which the script finds — yet it
// stayed in sight in instaCache, for a reason not established yet.
//
// Where the engine knows `:has()` (Chromium 105, Qt 6.5 and later), a rule
// hides it as well, before it is ever drawn and whatever React does to the
// element afterwards — which does not depend on the script at all. Qt 6.4 drops the rule as one it cannot parse, and the
// script alone does the work. Only for a link, which cannot hold another:
// a `[role=button]` ancestor could be a whole menu.
var labels = LABELS.alsoFromMeta.map(function (label) {
    return "svg[aria-label=" + JSON.stringify(label) + "]";
});
addStyle("instacache-mod-hide-meta",
    "a:has(" + labels.join(", ") + ") { display: none !important; }");

function hide(item) {
    if (item && item.style.getPropertyValue("display") !== "none")
        item.style.setProperty("display", "none", "important");
}

// The text, in whatever element holds it, for an item drawn without the
// labelled icon. A walk through the whole document, so not at every change
// of a page that changes all the time, as Reels does.
var TEXT_EVERY_MS = 3000;
var textAt = 0;

onPageChange(function () {
    var icons = document.querySelectorAll(labels.join(", "));
    for (var i = 0; i < icons.length; i++)
        hide(icons[i].closest("a, [role=button], [role=link], button"));
    var now = Date.now();
    if (icons.length > 0 || now - textAt < TEXT_EVERY_MS)
        return;
    textAt = now;
    for (var j = 0; j < LABELS.alsoFromMeta.length; j++) {
        var texts = document.evaluate(
            "//*[not(self::script)][normalize-space(text())="
                + JSON.stringify(LABELS.alsoFromMeta[j]) + "]",
            document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
        for (var k = 0; k < texts.snapshotLength; k++)
            hide(texts.snapshotItem(k).closest("a, [role=button], [role=link]"));
    }
});
