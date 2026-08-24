<div align="center">

<img src="../assets/instacache.svg" width="112" alt="instaCache">

# instaCache

**Un client Instagram natif et ultra-léger pour Linux.**

Un seul binaire de 517 Ko. Pas d'Electron, pas de Node, pas de Python.

[Installation](#installation) · [Pourquoi](#pourquoi) · [Raccourcis](#raccourcis-clavier) · [Configuration](#configuration) · [Compilation](#compiler-depuis-les-sources)

*[Read this in English](../README.md)*

</div>

---

## Ce que c'est

instaCache met Instagram dans une vraie fenêtre de bureau : l'application
apparaît dans ta barre des tâches, retient où tu l'avais laissée, te garde
connecté, et ne te dérange pas.

Techniquement, c'est une fenêtre GTK 3 qui contient une vue WebKitGTK — le même
moteur que Safari et GNOME Web — avec son cache et sa session ancrés dans des
dossiers persistants. Rien n'est embarqué, rien n'est dupliqué : c'est le moteur
de rendu déjà présent sur ton système qui travaille.

## Pourquoi

| | instaCache | Un wrapper Electron | Un onglet de navigateur |
|---|---|---|---|
| Taille du téléchargement | **517 Ko** | 80 à 150 Mo | — |
| Moteur de navigateur embarqué | aucun (utilise WebKitGTK) | un Chromium complet | — |
| Icône et fenêtre dédiées | oui | oui | non |
| Survit à la fermeture du navigateur | oui | oui | non |
| Session conservée entre deux lancements | oui | oui | oui |
| Cache disque réutilisé au redémarrage | oui, agressivement | en général | oui |

Mesuré sur la machine de référence : **433 Mo** de mémoire proportionnelle avec
le fil ouvert, répartis sur les trois processus WebKit. C'est à peu près ce que
coûte un seul onglet Instagram dans Chrome, et nettement moins qu'un wrapper
Electron — mais cela reste un moteur de navigateur complet qui affiche une
application web lourde, pas un jouet.

## Fonctionnalités

- **Cache disque agressif et persistant.** Le plus gros budget de cache de
  WebKit (`CacheModel::WebBrowser`) plus le cache de page avant/arrière, écrit
  dans `~/.cache/instacache` et réutilisé à chaque lancement. Un démarrage à
  chaud ne retélécharge pas l'interface.
- **Tu restes connecté.** Cookies, stockage local, IndexedDB et service workers
  vivent dans `~/.local/share/instacache` et survivent aux redémarrages et aux
  vidages de cache.
- **La fenêtre se souvient d'elle-même.** Taille, position, état maximisé et
  zoom sont restaurés — y compris quand la session de bureau se termine et que
  l'application est arrêtée plutôt que fermée.
- **Dimensionnée pour ton écran.** Le premier lancement occupe 90 % de la zone
  utile de ton moniteur au lieu d'une taille fixe, pour que rien ne soit coupé.
- **Vraie navigation au clavier.** Rechargement, rechargement forcé, précédent,
  suivant, accueil, zoom, plein écran.
- **Les liens externes sortent.** Tout ce qui n'est pas Instagram — ni l'un des
  domaines Meta nécessaires à la connexion — s'ouvre dans ton navigateur.
- **Notifications de bureau.** Les notifications web deviennent de vraies
  notifications ; un clic met la fenêtre au premier plan et prévient la page,
  qui ouvre alors la bonne conversation.
- **Plusieurs comptes en même temps.** `instacache --profile perso` obtient sa
  propre session, son propre cache et sa propre fenêtre, en parallèle du compte
  principal.
- **Une vraie page hors-ligne** au lieu de l'écran d'erreur par défaut de WebKit.

## Installation

### N'importe quelle distribution — l'installeur

Télécharge l'archive correspondant à ton architecture depuis la
[page des versions](https://git.justw.tf/LightZirconite/instaCache/releases),
puis :

```sh
tar -xzf instacache-1.0.0-linux-x86_64.tar.gz
cd instacache-1.0.0-linux-x86_64
./install.sh
```

C'est tout. L'installeur :

- installe dans `~/.local` — **aucun accès root nécessaire** ;
- ajoute instaCache à ton menu d'applications, avec son icône ;
- vérifie que la bibliothèque WebKitGTK 4.1 est présente et, si elle manque,
  affiche la commande exacte à lancer pour ta distribution.

Ensuite, lance **instaCache** depuis ton menu d'applications.

Autres possibilités :

```sh
./install.sh --system            # /usr/local, pour tous les utilisateurs
./install.sh --prefix ~/apps     # où tu veux
./install.sh --build             # compiler au lieu d'utiliser le binaire fourni
./install.sh --help
```

Pour désinstaller :

```sh
./uninstall.sh            # retire l'application, garde ta session
./uninstall.sh --purge    # supprime aussi session, cache et réglages
```

### Arch, CachyOS, Manjaro, EndeavourOS

```sh
git clone https://git.justw.tf/LightZirconite/instaCache.git
cd instaCache/packaging
makepkg -si
```

`makepkg -si` compile puis installe un vrai paquet système : l'application
apparaît dans le menu, et `sudo pacman -R instacache` la retire proprement.
La procédure de publication sur l'AUR est décrite dans
[docs/publishing.md](publishing.md).

### Dépendance à l'exécution

instaCache s'appuie sur les bibliothèques WebKitGTK 4.1 et GTK 3 déjà empaquetées
par ta distribution. Il n'embarque pas de navigateur.

| Distribution | Commande |
|---|---|
| Arch, CachyOS, Manjaro | `sudo pacman -S --needed webkit2gtk-4.1 gtk3` |
| Debian, Ubuntu, Mint | `sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0` |
| Fedora, RHEL | `sudo dnf install webkit2gtk4.1 gtk3` |
| openSUSE | `sudo zypper install libwebkit2gtk-4_1-0 gtk3` |
| Alpine | `sudo apk add webkit2gtk-4.1 gtk+3.0` |
| Void | `sudo xbps-install -S webkit2gtk gtk+3` |

### La lecture vidéo demande GStreamer

WebKit décode la vidéo via GStreamer, qui est empaqueté séparément. Instagram
diffuse du H.264 en MP4 : il faut donc le démultiplexeur MP4 (`qtdemux`), la
source HTTP (`souphttpsrc`) et la sortie audio. **Sans eux les photos
s'affichent normalement mais tous les Reels, Stories et vidéos restent vides** —
un symptôme qui ressemble à un bug de l'application alors qu'il s'agit d'un
paquet manquant.

| Distribution | Commande |
|---|---|
| Arch, CachyOS, Manjaro | `sudo pacman -S --needed gst-plugins-good gst-plugins-bad gst-libav` |
| Debian, Ubuntu, Mint | `sudo apt install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav` |
| Fedora, RHEL | `sudo dnf install gstreamer1-plugins-good gstreamer1-plugins-bad-free gstreamer1-libav` |
| openSUSE | `sudo zypper install gstreamer-plugins-good gstreamer-plugins-bad gstreamer-plugins-libav` |

`install.sh` vérifie à la fois WebKitGTK et les codecs, et affiche la commande
exacte pour ta distribution s'il manque quelque chose.

## Raccourcis clavier

| Raccourci | Action |
|---|---|
| `Ctrl+R` · `F5` | Recharger (le cache est utilisé — c'est le chemin rapide) |
| `Ctrl+Maj+R` · `Maj+F5` | Recharger en ignorant le cache |
| `Alt+←` · `Alt+→` | Précédent · Suivant |
| `Ctrl+H` · `Alt+Origine` | Revenir à ton fil |
| `Ctrl+=` · `Ctrl+-` · `Ctrl+0` | Zoom avant · arrière · réinitialiser |
| `F11` · `Échap` | Entrer · sortir du plein écran |
| `Ctrl+W` · `Ctrl+Q` | Quitter |
| `Ctrl+Maj+I` · `F12` | Inspecteur web (si activé dans la configuration) |

Le balayage à deux doigts sur un pavé tactile fait aussi précédent/suivant.

## Ligne de commande

```
instacache [OPTIONS] [URL]

  <URL>                  Ouvrir cette adresse Instagram au lieu de ton fil.
  -p, --profile <NOM>    Utiliser une session, un cache et une fenêtre séparés.
      --clear-cache      Supprimer le cache, rester connecté.
      --clear-session    Supprimer cookies et stockage (te déconnecte).
  -h, --help             Aide complète.
  -V, --version          Version.
```

Lancer instaCache deux fois avec le même profil met la fenêtre existante au
premier plan au lieu d'ouvrir une seconde copie.

## Configuration

`~/.config/instacache/config.json` est créé au premier lancement avec toutes les
options à leur valeur par défaut. Modifie-le puis relance l'application.

| Clé | Défaut | Rôle |
|---|---|---|
| `home_url` | `https://www.instagram.com/` | Page ouverte au démarrage et par `Ctrl+H`. |
| `user_agent` | une chaîne Safari macOS | Envoyé à Instagram. Une chaîne vide garde celui de WebKitGTK. |
| `hardware_acceleration` | `auto` | `auto`, `always` ou `never`. Mets `never` en cas de défaut d'affichage. |
| `developer_tools` | `false` | Active l'inspecteur web et la sortie console. |
| `notifications` | `true` | Transmettre les notifications web au bureau. |
| `open_external_links_in_browser` | `true` | Envoyer les liens non-Instagram au navigateur. |
| `spell_checking_languages` | `[]` | Par ex. `["fr_FR", "en_US"]`. Vide = correction désactivée. |
| `default_zoom` | `1.0` | Zoom utilisé quand aucun état de fenêtre n'est enregistré. |
| `remember_window_state` | `true` | Restaurer taille, position et zoom. |
| `show_loading_indicator` | `true` | La fine barre dégradée en haut de la fenêtre. |
| `start_maximized` | `false` | Toujours ouvrir en plein écran fenêtré. |

### Style personnalisé

Dépose du CSS dans `~/.config/instacache/user.css`, il est appliqué à toutes les
pages.

```css
/* Élargir le fil sur un grand écran */
main[role="main"] { max-width: 1100px; }
```

### Où sont tes données

| Chemin | Contenu | Suppression sans risque |
|---|---|---|
| `~/.config/instacache/` | `config.json`, `user.css`, géométrie de fenêtre | oui, remet les réglages à zéro |
| `~/.local/share/instacache/` | cookies, stockage local, IndexedDB — ta session | oui, te déconnecte |
| `~/.cache/instacache/` | le cache des ressources | oui, toujours |

Tous ces chemins respectent les variables `XDG_*_HOME`, et peuvent être
redirigés avec `INSTACACHE_DATA_HOME`, `INSTACACHE_CACHE_HOME` et
`INSTACACHE_CONFIG_HOME` pour une installation portable.

## Compiler depuis les sources

```sh
sudo pacman -S --needed rust webkit2gtk-4.1 gtk3 pkgconf   # ou l'équivalent
git clone https://git.justw.tf/LightZirconite/instaCache.git
cd instaCache
cargo build --release
./install.sh
```

Il te faut les paquets `-dev` / `-devel` de GTK 3 et WebKitGTK 4.1 pour
compiler — `libgtk-3-dev` et `libwebkit2gtk-4.1-dev` sur Debian et Ubuntu.

### Vérifier qu'une page s'affiche vraiment

Les captures d'écran du serveur d'affichage ne sont pas fiables sur certaines
configurations Wayland et XWayland. Cet utilitaire fait le rendu via WebKit
lui-même et écrit un PNG, donc il fonctionne partout :

```sh
cargo run --example snapshot -- https://www.instagram.com/ capture.png
```

Il utilise la vraie configuration de l'application et un profil jetable : ta
session n'est jamais touchée.

## Publier une version

```sh
scripts/release.sh patch --dry-run   # aperçu
scripts/release.sh patch             # version, changelog, commit, tag, push
```

Le push du tag déclenche `.github/workflows/release.yml`, qui compile les
archives x86_64 et aarch64 puis publie la release avec ses notes et ses sommes
de contrôle. Le workflow tourne aussi bien sur GitHub Actions que sur Gitea
Actions.

## Architecture

```
src/
  main.rs        analyse des arguments et démarrage du processus
  lib.rs         câblage des modules et constantes de l'application
  ui.rs          fenêtre, signaux, notifications, téléchargements
  web.rs         contexte WebKit, stockage persistant, réglages, liens
  config.rs      config.json et géométrie de la fenêtre
  paths.rs       emplacements XDG et profils
  shortcuts.rs   navigation au clavier
  urls.rs        quels domaines restent dans l'application
  errorpage.rs   la page hors-ligne
examples/
  snapshot.rs    rendu d'une page en PNG via WebKit, pour vérification
```

Le crate est séparé en une bibliothèque et un binaire mince pour que
l'utilitaire de capture exerce exactement la configuration livrée.

## État du projet et limites

- instaCache affiche le site d'Instagram lui-même. Si Instagram change quelque
  chose, instaCache suit automatiquement — mais il hérite aussi de ce
  qu'Instagram ne propose pas sur le web.
- Les demandes d'accès caméra, micro, géolocalisation et verrouillage du
  pointeur sont refusées d'office. L'application web n'en a pas besoin.
- Ceci est un client non officiel, sans aucun lien avec Instagram ni Meta.
  Instagram est une marque de Meta Platforms, Inc.

## Licence

[MIT](../LICENSE).
