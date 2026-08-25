<div align="center">

<img src="../assets/instacache.svg" width="112" alt="instaCache">

# instaCache

**Un client Instagram natif et ultra-léger pour Linux.**

Un seul binaire de 2,1 Mo. Pas d'Electron, pas de Node, pas de Python.

[Installation](#installation) · [Pourquoi](#pourquoi) · [Raccourcis](#raccourcis-clavier) · [Configuration](#configuration) · [Compilation](#compiler-depuis-les-sources)

*[Read this in English](../README.md)*

</div>

---

## Ce que c'est

instaCache met Instagram dans une vraie fenêtre de bureau : l'application
apparaît dans ta barre des tâches, retient où tu l'avais laissée, te garde
connecté, et ne te dérange pas.

Techniquement, c'est une fenêtre Qt Quick qui contient une vue Qt WebEngine —
le Chromium que ta distribution empaquette déjà, partagé avec toutes les autres
applications Qt de la machine — avec son cache et sa session ancrés dans des
dossiers persistants. Rien n'est embarqué, rien n'est dupliqué : c'est le
moteur de rendu déjà présent sur ton système qui travaille.

## Pourquoi

| | instaCache | Un wrapper Electron | Un onglet de navigateur |
|---|---|---|---|
| Taille du téléchargement | **2,1 Mo** | 80 à 150 Mo | — |
| Moteur de navigateur embarqué | aucun (utilise le Qt WebEngine du système) | un Chromium complet | — |
| Icône et fenêtre dédiées | oui | oui | non |
| Survit à la fermeture du navigateur | oui | oui | non |
| Session conservée entre deux lancements | oui | oui | oui |
| Cache disque réutilisé au redémarrage | oui, agressivement | en général | oui |

Mesuré sur la machine de référence, en additionnant la mémoire proportionnelle
de tous les processus : environ **300 Mo** sur une fenêtre fraîche, et **800 Mo**
sur un fil connecté après une minute de défilement avec des vidéos. C'est le
second chiffre qui compte au quotidien.

C'est le prix d'un moteur de navigateur affichant une application web lourde :
instaCache est petit, l'application web ne l'est pas. Ce que tu gagnes, c'est un
téléchargement de 2,1 Mo, l'absence d'un second moteur de navigateur sur ton
disque, et un cache qui rend le lancement suivant instantané. Ce n'est pas une
façon légère de *regarder* Instagram — c'est une enveloppe légère *autour*
d'Instagram.

## Fonctionnalités

- **Cache disque agressif et persistant.** Le cache HTTP sur disque de
  Chromium, écrit dans `~/.cache/instacache` et réutilisé à chaque lancement.
  Un démarrage à chaud ne retélécharge pas l'interface.
- **Tu restes connecté.** Cookies, stockage local, IndexedDB et service workers
  vivent dans `~/.local/share/instacache` et survivent aux redémarrages et aux
  vidages de cache.
- **La fenêtre se souvient d'elle-même.** Taille, position, état maximisé et
  zoom sont restaurés — y compris quand la session de bureau se termine et que
  l'application est arrêtée plutôt que fermée.
- **Dimensionnée pour ton écran.** Le premier lancement occupe 90 % de la zone
  utile de ton moniteur au lieu d'une taille fixe, pour que rien ne soit coupé.
- **Une barre de chargement qui suit vraiment Instagram.** Une fine ligne
  dégradée en haut, comme le fait YouTube. Elle suit les vrais chargements de
  page *et* la navigation interne, qui ne déclenche aucun chargement de page et
  laisserait sinon la barre inerte.
- **Une vidéo qui ne saccade pas.** Chromium réutilise ses décodeurs au lieu
  d'en reconstruire un par clip, ce qu'un fil de Reels lui demande environ deux
  fois par seconde. Mesuré sur la machine de référence : **1 à 6** images en
  retard par exécution de 40 s (fourchette sur cinq exécutions), là où le
  moteur WebKitGTK utilisé jusqu'ici en produisait **78**. Le décodage VA-API est activé au passage, ce que Chromium
  laisse désactivé par défaut sous Linux.
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
- **Une vraie page hors-ligne** au lieu de l'écran d'erreur par défaut de Chromium.

## Installation

Une seule commande. Colle-la dans un terminal :

```sh
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh
```

Elle télécharge la version correspondant à ton architecture, vérifie sa somme
SHA-256 publiée, installe instaCache dans `~/.local` — **aucun accès root
nécessaire pour l'application elle-même** — et l'ajoute à ton menu.

Elle vérifie aussi les deux bibliothèques système dont instaCache a besoin et
propose d'installer celles qui manquent avec le gestionnaire de paquets de ta
distribution. Cette étape-là demande ton mot de passe, parce qu'installer des
paquets système l'exige. Tu réponds `y` et c'est réglé ; la commande exacte
s'affiche avant d'être lancée, pour que tu voies ce qui va s'exécuter.

Ensuite, lance **instaCache** depuis ton menu d'applications.

<details>
<summary>Options</summary>

```sh
# Installer pour tous les utilisateurs
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh -s -- --system

# Ne rien demander, installer les paquets manquants automatiquement
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh -s -- --yes

# Installer seulement l'application, sans toucher aux paquets système
curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh | sh -s -- --no-deps

# Une version précise
INSTACACHE_VERSION=v1.0.0 sh -c "$(curl -fsSL https://raw.githubusercontent.com/LightZirconite/instaCache/main/get.sh)"
```

</details>

<details>
<summary>Tu préfères ne pas envoyer un script dans un shell ?</summary>

Télécharge l'archive toi-même depuis la
[page des versions](https://github.com/LightZirconite/instaCache/releases),
puis :

```sh
tar -xzf instacache-*-linux-x86_64.tar.gz
cd instacache-*-linux-x86_64
./install.sh
```

Même installeur, même résultat. `get.sh` ne fait qu'automatiser le
téléchargement et vérifier la somme de contrôle à ta place.

</details>

## Mises à jour

Rien d'autre ne met instaCache à jour — il est installé depuis une archive, pas
par un gestionnaire de paquets — donc il se met à jour lui-même.

Au démarrage, il demande une fois par jour à GitHub s'il existe une version plus
récente. Si oui, et que ton installation est dans `~/.local` où aucun accès root
n'est nécessaire, elle est téléchargée, vérifiée par sa somme SHA-256 et
installée en arrière-plan. Tu reçois une notification t'invitant à relancer.
Rien n'est jamais remplacé pendant que tu l'utilises.

Pour vérifier tout de suite :

```sh
instacache --update
```

Une installation système ne peut pas se mettre à jour sans root : elle se
contente alors de te signaler qu'une nouvelle version existe. Pour tout
désactiver, mets `"auto_update": false` dans `config.json`.

## Désinstallation

L'installeur laisse le désinstalleur à côté de l'application, donc ça marche
même si tu as utilisé la commande unique et que tu n'as plus l'archive :

```sh
~/.local/share/instacache/uninstall.sh            # retire l'app, garde ta session
~/.local/share/instacache/uninstall.sh --purge    # supprime aussi session, cache et réglages
```

Pour une installation `--system`, le chemin est
`/usr/local/share/instacache/uninstall.sh`.

## Ce dont l'application a besoin sur ton système

instaCache n'embarque pas de navigateur. Il utilise le Qt WebEngine déjà
empaqueté par ta distribution — le même Chromium que toutes les autres
applications Qt de la machine — et l'installeur s'en occupe pour toi. Pour
référence :

| | Paquet (Arch) | Paquet (Debian/Ubuntu) | Sans lui |
|---|---|---|---|
| Rendu | `qt6-webengine qt6-declarative` | `libqt6webenginequick6 qml6-module-qtwebengine` | Ne démarre pas |
| Vidéo H.264 | inclus | inclus | — |

Qt 6.4 ou plus récent, c'est-à-dire ce que Debian 12 et tout ce qui suit
embarquent.

**Fedora fait exception.** Sa version de Qt WebEngine est compilée sans les
codecs soumis à brevet, et Instagram est en H.264 de bout en bout : les photos
s'affichent et aucune vidéo ne charge tant que `qt6-qtwebengine-freeworld`
n'est pas installé. `./install.sh` le détecte et le corrige.

## Performance vidéo

Un fil de Reels demande au navigateur de construire puis de jeter une vidéo
toutes les demi-secondes. C'est ça qui fait saccader un fil, et c'est là que
les moteurs diffèrent le plus. Mesuré sur la machine de référence — quatre flux
H.264 1080×1920 à 30 i/s, un remplacé toutes les 500 ms, deux exécutions
concordantes chacune :

| moteur | images > 50 ms | images affichées | 1re image |
|---|---|---|---|
| **Qt WebEngine** (ce qui est livré) | **1 à 6** | 4176–4777 | **48–64 ms** |
| WebKitGTK 4.1 (utilisé jusqu'à la 1.2.0) | 78 | 4720 | 264 ms |

La fourchette porte sur cinq exécutions, pas sur la meilleure.

C'est la raison du changement de moteur. WebKit construit un pipeline GStreamer
neuf pour chaque `<video>`, sur le fil d'exécution qui fait aussi tourner la
page ; Chromium réutilise ses décodeurs. Aucun réglage WebKit n'a comblé
l'écart, et ceux qui en donnaient l'illusion sont listés, chiffres à l'appui,
dans [`bench/`](../bench/README.md) — tu peux tout reproduire toi-même, sur ta
machine, en deux minutes.

Deux réglages restent si la lecture se comporte mal. `video_decoding` choisit
le décodeur : `gpu` (le défaut) active VA-API, que Chromium désactive sous
Linux ; `software` laisse le décodage au processeur ; `auto` s'en remet à
Chromium. `hardware_acceleration: never` coupe le GPU entièrement, en dernier
recours si la fenêtre s'affiche mal.

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
      --add-site <NOM> <URL>
                         Ajouter un site à ton menu d'applications.
      --icon <CHEMIN>    Utiliser cette image au lieu du logo du site.
      --remove-site <NOM>
                         L'en retirer. Ses données sont conservées.
      --list-sites       Lister les sites ajoutés.
      --update           Chercher une version plus récente et l'installer.
      --clear-cache      Supprimer le cache, rester connecté.
      --clear-session    Supprimer cookies et stockage (te déconnecte).
  -h, --help             Aide complète.
  -V, --version          Version.
```

Lancer instaCache deux fois avec le même profil met la fenêtre existante au
premier plan au lieu d'ouvrir une seconde copie.

## Un autre site, dans sa propre fenêtre

instaCache est un client Instagram, mais rien dans la fenêtre n'est propre à
Instagram : c'est un moteur pointé sur un site. Pointe-le ailleurs et il devient
l'application de ce site :

```sh
instacache --add-site X https://x.com/ --domains x.com,twimg.com
```

Ça écrit deux choses : un profil, et une entrée de menu. **X apparaît alors dans
ton menu d'applications avec le logo X**, et s'ouvre dans sa propre fenêtre,
avec sa session, son cache et ses cookies — être connecté à X n'a rien à voir
avec être connecté à Instagram.

L'icône vient du site lui-même : celle qu'il déclare dans son code, la plus
grande, et son `/favicon.ico` sinon. GitHub en publie une en 512×512 et c'est
celle-là qui est prise ; X n'en déclare aucune, donc son favicon est utilisé.
Le format est déduit du contenu et non du nom du fichier, parce que
`x.com/favicon.ico` est en réalité un PNG. Quand un site ne publie rien
d'exploitable, l'icône d'instaCache est conservée, et `--icon chemin/logo.png`
remplace tout ça.

`--domains` est la liste blanche de cette fenêtre, et elle compte : un site dont
les images viennent d'un autre hôte a besoin que cet hôte soit nommé, sinon les
images sont traitées comme des liens externes. Omis, il vaut l'hôte de l'adresse
donnée, ce qui suffit pour un site qui sert tout lui-même :

```sh
instacache --add-site "Hacker News" https://news.ycombinator.com/
```

`--list-sites` montre ce que tu as ajouté et `--remove-site X` retire l'entrée du
menu. Retirer une entrée ne supprime jamais la session derrière ;
`instacache --profile x --clear-session` s'en charge.

## Configuration

`~/.config/instacache/config.json` est créé au premier lancement avec toutes les
options à leur valeur par défaut. Modifie-le puis relance l'application.

| Clé | Défaut | Rôle |
|---|---|---|
| `home_url` | `https://www.instagram.com/` | Page ouverte au démarrage et par `Ctrl+H`. |
| `user_agent` | une chaîne Chrome Linux | Envoyé à Instagram. Honnête sur le système *et* sur le moteur : annoncer Safari mettrait un moteur Chromium sur le code Safari. Vide = celui de Qt WebEngine. |
| `hardware_acceleration` | `always` | `always`, `auto` ou `never`. Les deux premiers laissent Chromium décider lui-même. Mets `never` seulement si la fenêtre s'affiche mal : ça coupe entièrement la composition GPU. |
| `video_decoding` | `gpu` | `gpu`, `software` ou `auto`. Voir [Performance vidéo](#performance-vidéo). |
| `allow_autoplay_with_sound` | `true` | Autoriser une vidéo à démarrer avec le son. Sinon le moteur coupe le son de tout ce qui démarre sans clic, ce qui se lit comme « l'app se mute toute seule ». |
| `context_menu` | `false` | Afficher le menu du clic droit. Désactivé : dans une fenêtre dédiée à une seule application c'est du chrome de navigateur — Précédent, Suivant, Code source — et ça recouvre la page. Le désactiver retire aussi « Enregistrer l'image » et « Copier l'adresse du lien » ; mets `true` pour les retrouver. |
| `developer_tools` | `false` | Active l'inspecteur web et la sortie console. |
| `notifications` | `true` | Transmettre les notifications web au bureau. |
| `open_external_links_in_browser` | `true` | Envoyer les liens non-Instagram au navigateur. |
| `internal_domains` | Instagram + les hôtes Meta nécessaires au login | Les hôtes autorisés à s'afficher dans la fenêtre, en liste blanche — un hôte ne correspond qu'exactement ou comme sous-domaine. Par profil : un second profil peut donc être une fenêtre dédiée à un autre site, en pointant `home_url` dessus et en nommant ses domaines ici. Threads n'y est volontairement pas ; ajoute `threads.com` pour le garder dans la fenêtre. Une liste vide restaure le défaut au lieu de verrouiller la fenêtre. |
| `spell_checking_languages` | `[]` | Par ex. `["fr_FR", "en_US"]`. Vide = correction désactivée. |
| `default_zoom` | `1.0` | Zoom utilisé quand aucun état de fenêtre n'est enregistré. |
| `remember_window_state` | `true` | Restaurer taille, position et zoom. |
| `show_loading_indicator` | `true` | La fine barre dégradée en haut de la fenêtre. |
| `start_maximized` | `false` | Toujours ouvrir en plein écran fenêtré. |
| `auto_update` | `true` | Chercher une nouvelle version sur GitHub et l'installer. |
| `update_check_interval_hours` | `24` | Heures entre deux vérifications. `0` = à chaque lancement. |

### Style et script personnalisés

Dépose du JavaScript dans `~/.config/instacache/user.js` : il s'exécute sur
chaque page une fois chargée, dans le monde de la page, donc il voit et modifie
ce que la page voit.

```js
// Masquer la colonne de suggestions
document.querySelectorAll('aside').forEach(el => el.remove());
```

**Il n'y a pas d'extensions, et il ne peut pas y en avoir.** Qt WebEngine
n'implémente aucune API d'extension — ni Chrome Web Store, ni `.crx`, ni
uBlock. `user.js` est ce qui s'en rapproche le plus, et il n'est délibérément
pas mis en bac à sable : c'est ton fichier, exécuté avec les droits de la page.
Une erreur dedans est attrapée et signalée dans la console au lieu de casser la
page, mais pour le reste il est cru sur parole. Ne colle pas un script que tu
n'as pas lu.

Dépose du CSS dans `~/.config/instacache/user.css`, il est appliqué à toutes les
pages.

```css
/* Élargir le fil sur un grand écran */
main[role="main"] { max-width: 1100px; }
```

### Mise à jour depuis une version WebKitGTK

instaCache utilisait WebKitGTK jusqu'à la 1.2.0 et Qt WebEngine à partir de la
2.0.0. Un moteur Chromium ne sait pas lire le pot à cookies de WebKit : **le
premier lancement après cette mise à jour te redemande donc de te connecter**.
Rien d'autre n'est perdu — réglages, géométrie de la fenêtre et profils sont
conservés.

Les fichiers de l'ancien moteur restent en place, inutilisés, dans
`~/.local/share/instacache` : `cookies.sqlite`, `localstorage/`,
`serviceworkers/`, `storage/` et `mediakeys/`. Les supprimer est sans risque.

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
sudo pacman -S --needed rust qt6-webengine qt6-declarative pkgconf  # ou l'équivalent
git clone https://git.justw.tf/LightZirconite/instaCache.git
cd instaCache
cargo build --release
./install.sh
```

Il te faut les paquets `-dev` / `-devel` de Qt 6 Base, Qt 6 Declarative et Qt 6
WebEngine pour compiler — `qt6-base-dev qt6-declarative-dev qt6-webengine-dev`
sur Debian et Ubuntu. La compilation les trouve via `qmake6`, qui doit donc
être dans le `PATH`.

### Vérifier qu'une page s'affiche vraiment

Les captures d'écran du serveur d'affichage ne sont pas fiables sur certaines
configurations Wayland et XWayland. Cet utilitaire capture la vue depuis
l'intérieur du moteur et écrit un PNG, donc il fonctionne partout :

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
  main.rs        arguments, démarrage, signaux de terminaison
  lib.rs         câblage des modules et constantes de l'application
  bridge.rs      tout ce que QML a le droit de demander à Rust — la politique
  qml/main.qml   la fenêtre, la vue, la barre de chargement, les raccourcis
  chromium.rs    les réglages traduits en drapeaux Chromium
  config.rs      config.json et géométrie de la fenêtre
  paths.rs       emplacements XDG et profils
  downloads.rs   où va un téléchargement et sous quel nom
  instance.rs    une fenêtre par profil, via une socket Unix
  urls.rs        quels domaines restent dans l'application
  errorpage.rs   la page hors-ligne
  updates.rs     recherche et installation d'une nouvelle version
examples/
  snapshot.rs    rendu d'une page en PNG, pour vérification
  stress.rs      pilotage d'une page depuis l'intérieur, pour les plantages
bench/           le banc de mesure de fluidité vidéo
```

La séparation est volontaire : la scène QML ne possède que des widgets, et
chaque décision dont elle a besoin — cette URL est-elle interne, où va ce
téléchargement, faut-il recharger après un plantage du moteur de rendu — est
tranchée par Rust, où elle est testée unitairement. La scène est compilée dans
le binaire : il n'y a toujours qu'un seul fichier à livrer.

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
