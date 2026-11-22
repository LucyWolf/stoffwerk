# Stoffwerk

Kleidung für VRChat-Avatare – ohne Abo, auch unter Linux.

Du zeichnest Schnittteile in 2D wie beim echten Nähen, legst fest, welche Kanten zusammengenäht werden, und lässt den Stoff auf deinen Avatar fallen. Heraus kommt ein Kleidungsstück, das direkt in Unity weiterverwendet werden kann.

**Was anders ist als bei vergleichbarer Software**
- **Spieltaugliches Netz ohne Retopologie.** Stoffwerk vernetzt die Schnittteile selbst mit sauberen Vierecken; die UVs ergeben sich direkt aus dem Schnitt. Das Netz aus der Simulation ist schon das fertige Spielnetz.
- **Fertig für VRChat.** Avatar als VRM laden, beim Export werden die Gewichte vom Skelett übernommen – die Kleidung bewegt sich mit.
- **Linux und Windows, kostenlos.**

> **Stand:** frühe Entwicklung. Bisher: Avatar laden und ansehen, Schnittteile zeichnen und Nähte festlegen. Simulation und Export folgen.

## Installieren

Auf der [Release-Seite](https://github.com/LucyWolf/stoffwerk/releases) liegen:

| Datei | Für |
|---|---|
| `Stoffwerk-…-setup.exe` | Windows: Doppelklick, fertig |
| `.desktop`-Dateien | Linux: Doppelklick installiert Stoffwerk ins Anwendungsmenü |
| `stoffwerk-linux` | Linux: das Programm selbst, ausführbar machen und starten |

Gebraucht wird eine Grafikkarte mit Vulkan (Linux) bzw. DirectX 12 oder Vulkan (Windows).

## Bedienung

| | |
|---|---|
| Avatar laden | Datei → Avatar laden, oder Datei ins Fenster ziehen |
| Bildschirmfoto | Datei → Bildschirmfoto speichern (PNG, doppelte Auflösung) |
| 3D drehen | linke Maustaste ziehen |
| 3D verschieben | rechte oder mittlere Maustaste ziehen |
| Zoomen | Mausrad |
| Auf Avatar ausrichten | Doppelklick in die 3D-Ansicht |
| Schnittfläche verschieben | rechte/mittlere Maustaste, oder Leertaste + linke |

### Schnitt-Werkzeuge

| Werkzeug | Taste | |
|---|---|---|
| Auswählen | A | Teile auswählen und verschieben |
| Punkte | Z | Punkte und Kanten ziehen; Rechtsklick: Punkt löschen, Abnäher, Kurvenpunkt |
| + Punkt | X | Klick auf eine Kante setzt dort einen Punkt |
| Polygon | H | Punkte klicken (Umschalt = Kurvenpunkt), erster Punkt oder Enter schließt |
| Rechteck / Kreis | S / E | aufziehen; Umschalt = exakter Kreis |
| Innenlinie | I | in ein Teil klicken, Enter oder Doppelklick beendet |
| Naht | N | Kante, dann Gegenkante – jeweils nahe dem Ende, das zusammengehört |

Rechtsklick auf ein Teil: duplizieren, **symmetrisch kopieren** (die Kopie folgt jeder Änderung gespiegelt), spiegeln, drehen, löschen. Strg+Z / Strg+Y für Rückgängig und Wiederholen, Entf löscht die Auswahl.

## Dateiformate

**Avatar laden:** FBX, VRM, GLB/glTF, OBJ (+MTL), Collada (DAE), USD (USD/USDA/USDC/USDZ), Alembic (ABC)

**Texturen:** PNG, JPG, TGA, TIF, BMP, GIF, EXR, WebP, PSD – eingebettet oder als Datei. Steht in der Avatar-Datei ein Pfad vom Rechner der Person, die den Avatar gebaut hat, sucht Stoffwerk die Textur über den Dateinamen im Avatar-Ordner.

Achsen und Einheiten werden beim Laden angeglichen: Der Avatar steht immer aufrecht und in echter Größe, egal ob die Datei Z oben oder Zentimeter verwendet.

Kleidung speichern (FBX, OBJ, glTF, Alembic, USD) kommt mit dem Export.

**Fehlersuche:** `stoffwerk --pruefen datei.fbx` lädt eine Datei ohne Fenster und zeigt, was gefunden wurde.

## Selbst bauen

```sh
cargo build --release
./target/release/stoffwerk
```

## Lizenz

MIT
