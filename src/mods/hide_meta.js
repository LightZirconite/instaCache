// Hides "Also from Meta" in Instagram's side bar: a menu of Meta's other
// apps, which is advertising rather than navigation. Found by its icon's
// label, or failing that by its text, and hidden — never removed, so
// Instagram's own code finds the element where it left it.
//
// Every copy is hidden, at every change of the page. The owner's logged-in
// page, captured in French, held one: a link (`a[role=link]`) around the
// labelled icon. It stayed in sight through 2.12.0 all the same, the exact
// rule included — so the label is compared loosely (`plain` in page.js), and
// the rule matches its first and last words rather than every character in
// between, where a non-breaking space would not match a plain one.
//
// Where the engine knows `:has()` (Chromium 105, Qt 6.5 and later), the rule
// hides it before it is ever drawn, whatever React does to the element
// afterwards. Qt 6.4 drops the rule as one it cannot parse, and the script
// alone does the work. Only for a link, which cannot hold another: a
// `[role=button]` ancestor could be a whole menu.
addStyle("instacache-mod-hide-meta",
    "a:has(svg[aria-label^='Also from'][aria-label$='Meta'])," +
    " a:has(svg[aria-label^='Autres applications'][aria-label$='Meta'])" +
    " { display: none !important; }");

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
    var found = 0;
    var icons = document.querySelectorAll("svg[aria-label]");
    for (var i = 0; i < icons.length; i++) {
        if (labelled(icons[i], LABELS.alsoFromMeta)) {
            hide(icons[i].closest("a, [role=button], [role=link], button"));
            found++;
        }
    }
    var now = Date.now();
    if (found > 0 || now - textAt < TEXT_EVERY_MS)
        return;
    textAt = now;
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    for (var node = walker.nextNode(); node; node = walker.nextNode()) {
        if (LABELS.alsoFromMeta.indexOf(plain(node.nodeValue)) >= 0)
            hide(node.parentElement.closest("a, [role=button], [role=link]"));
    }
});
