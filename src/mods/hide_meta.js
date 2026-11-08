// Hides "Also from Meta" in Instagram's side bar: a menu of Meta's other
// apps, which is advertising rather than navigation. Found by its icon's
// label, or failing that by its text, and hidden — never removed, so
// Instagram's own code finds the element where it left it.
var hidden = null;

function find() {
    var item = control(document, LABELS.alsoFromMeta);
    if (item)
        return item;
    for (var i = 0; i < LABELS.alsoFromMeta.length; i++) {
        var text = document.evaluate(
            "//span[normalize-space(text())=" + JSON.stringify(LABELS.alsoFromMeta[i]) + "]",
            document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;
        if (text)
            return text.closest("a, [role=button], [role=link]");
    }
    return null;
}

onPageChange(function () {
    if (hidden && hidden.isConnected)
        return;
    hidden = find();
    if (hidden)
        hidden.style.setProperty("display", "none", "important");
});
