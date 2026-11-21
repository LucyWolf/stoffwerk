# Stoffwerk

Kleidung für VRChat-Avatare – ohne Abo, auch unter Linux.

Du zeichnest Schnittteile in 2D wie beim echten Nähen, legst fest, welche Kanten zusammengenäht werden, und lässt den Stoff auf deinen Avatar fallen. Heraus kommt ein Kleidungsstück, das direkt in Unity weiterverwendet werden kann.

**Was anders ist als bei vergleichbarer Software**
- **Spieltaugliches Netz ohne Retopologie.** Stoffwerk vernetzt die Schnittteile selbst mit sauberen Vierecken; die UVs ergeben sich direkt aus dem Schnitt. Das Netz aus der Simulation ist schon das fertige Spielnetz.
- **Fertig für VRChat.** Avatar als VRM laden, beim Export werden die Gewichte vom Skelett übernommen – die Kleidung bewegt sich mit.
- **Linux und Windows, kostenlos.**

> **Stand:** frühe Entwicklung. Bisher: Avatar laden und ansehen. Schnitt, Simulation und Export folgen.

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
| Avatar laden | Datei → Avatar laden, oder Datei ins Fenster ziehen (FBX, VRM, GLB, glTF, OBJ) |
| 3D drehen | linke Maustaste ziehen |
| 3D verschieben | rechte oder mittlere Maustaste ziehen |
| Zoomen | Mausrad |
| Auf Avatar ausrichten | Doppelklick in die 3D-Ansicht |
| Schnittfläche verschieben | rechte/mittlere Maustaste, oder Leertaste + linke |

## Selbst bauen

```sh
cargo build --release
./target/release/stoffwerk
```

## Lizenz

MIT
