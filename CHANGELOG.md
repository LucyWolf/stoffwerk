# Changelog

## 0.1.2

**Aufteilung wie bei Marvelous Designer**
- 3D-Ansicht links, 2D-Schnitt in der Mitte, rechts Objektliste und Eigenschaften. Über jeder Ansicht eine eigene Leiste. In der 3D-Leiste gibt es feste Blickrichtungen (Vorne, Hinten, Links, Rechts, Oben).
- In der Objektliste kann man den Avatar ein- und ausblenden. Die Eigenschaften zeigen Größe, Dreiecke, Texturen und Hinweise wie fehlende Texturen.

**Avatare mit Texturen**
- Avatare erscheinen mit ihren echten Texturen statt grau. Haare, Wimpern und Spitze werden an den durchsichtigen Stellen ausgestanzt.
- Texturpfade in FBX und OBJ stammen oft vom Rechner der Person, die den Avatar gebaut hat. Stoffwerk sucht die Textur dann über den Dateinamen im Avatar-Ordner und darüber. Hat Unity eine .psd als .png abgelegt, wird die auch gefunden.
- Bildformate: PNG, JPG, TGA, TIF, BMP, GIF, EXR, WebP, PSD.

**Mehr Formate zum Laden**
- Collada (.dae), USD (.usd/.usda/.usdc/.usdz) und Alembic (.abc), zusätzlich zu FBX, VRM, glTF und OBJ.
- Collada und USD dürfen Z oben und Zentimeter verwenden, der Avatar steht trotzdem richtig.

**Sonstiges**
- Laden läuft im Hintergrund. Große Avatare mit vielen Texturen lassen das Fenster nicht mehr hängen.
- Datei > Bildschirmfoto speichern: die 3D-Ansicht als PNG in doppelter Auflösung.
- `stoffwerk --pruefen <datei>` lädt ohne Fenster und berichtet. Damit lassen sich Dateien prüfen, die nicht laden wollen. Der Release-Workflow lädt damit bei jedem Release eine Testfigur in jedem Format.

**Nicht geprüft**
- Alembic: keine Testdatei vorhanden. Gelesen werden nur die Netze ohne Transformationen und UVs, mehr kann die Bibliothek nicht.
- USDZ mit Texturen im Archiv: Die Texturen darin werden nicht gefunden, das Netz schon.
- Mit echten VRChat-Avataren noch nicht getestet, nur mit selbst gebauten Testfiguren.

## 0.1.1

**FBX-Avatare laden**
- FBX-Dateien öffnen jetzt direkt, so wie man es von Marvelous Designer kennt. Vorher kam nur ein Hinweis, die Datei erst umzuwandeln.
- Achsen und Einheiten werden beim Laden angeglichen: Exporte aus Blender oder 3ds Max haben oft Z oben und Zentimeter, im Programm steht der Avatar trotzdem aufrecht und in echter Größe. Sonst läge er auf dem Rücken und die Stoffsimulation wäre später um den Faktor 100 falsch skaliert.
- Gelesen wird mit ufbx, das Binär- und Text-FBX ab Version 6 versteht.

**Kleinigkeiten**
- Im Fehlerfenster und in der Statusleiste standen Kästchen statt Pfeilen; die eingebaute Schrift hat das Zeichen nicht.

**Nicht geprüft**
- Getestet mit einer selbst gebauten Text-FBX (Z oben, Zentimeter). Mit einem echten VRChat-Avatar als Binär-FBX noch nicht.

## 0.1.0

Erster Stand (Abschnitt 1 von 5): das Grundgerüst, auf dem Schnitt, Simulation und Export aufbauen.

**Avatar laden**
- VRM, GLB/glTF und OBJ, per Menü, per Ziehen ins Fenster oder als Startargument.
- Modelle, die in Zentimetern gespeichert sind (typisch für OBJ aus manchen Programmen), werden auf Meter umgerechnet; die Füße stehen auf dem Boden. Ohne das wäre die Stoffsimulation später um den Faktor 100 falsch skaliert.
- FBX geht noch nicht – das Programm sagt das mit Hinweis, wie man nach glTF exportiert.

**3D-Ansicht**
- Kamera wie in Blender: drehen, verschieben, zoomen, Doppelklick richtet auf den Avatar aus.
- Bodengitter in 10-cm-Schritten, Kantenglättung (4× MSAA).

**Schnittansicht**
- Leere Zeichenfläche mit Zentimeter-Raster, Verschieben und Zoomen zur Maus hin. Das Zeichnen der Schnittteile kommt in Abschnitt 2.

**Nicht geprüft**
- Nur unter Linux mit Software-Grafik (llvmpipe) getestet, noch nicht auf einer echten Grafikkarte und nicht unter Windows.
- Texturen des Avatars werden noch nicht angezeigt, er erscheint einfarbig grau.
