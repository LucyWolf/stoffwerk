# Changelog

## 0.1.9

**Stoff fiel durch den Avatar und knüllte**
- Die Kollision hat angenommen, dass alle Flächen eines Avatars nach außen zeigen. Bei Teilen mit falsch herum zeigenden Flächen und bei einseitigen Flächen (Ohren, Schnurrhaare, Wimpern) wurde der Stoff deshalb in den Körper hineingeschoben statt hinaus. Er fiel durch, verhakte sich und landete als Knäuel. Jetzt bleibt jeder Stoffpunkt auf der Seite einer Fläche, von der er gekommen ist, egal wohin die Fläche zeigt.
- Selbstkollision: Stoff kann nicht mehr durch sich selbst hindurch. Falten legen sich aufeinander statt ineinander. Auf einer Testfigur mit Katzenkopf, Ohren und Schnurrhaaren steckte das Tuch vorher an 232 Stellen in sich selbst, jetzt an keiner.
- Mehr Luftwiderstand: Der Stoff kommt zur Ruhe, statt lange nachzuzappeln.
- Baumwolle ist etwas steifer und weniger klebrig. Sie bildet größere, weichere Falten und rutscht leichter über den Körper.

**Prüfung**
- Neue Testfigur `figur_tpose_katze.obj` (T-Pose, runder Kopf, Ohren und Schnurrhaare als einzelne dünne Flächen). Ein fester Test lässt ein Tuch darauf fallen. Es muss auf dem Kopf bleiben, darf nicht bis zum Boden hängen und darf nicht in sich selbst stecken.

**Nicht geprüft**
- Mit Lucys echtem Avatar, nur mit der nachgebauten Testfigur.

## 0.1.8

Abschnitt 3 beginnt: Die Stoffsimulation.

**Schnittteile in 3D**
- Jedes Schnittteil erscheint in der 3D-Ansicht, vernetzt mit etwa 2 cm Punktabstand (einstellbar unter Simulation > Punktabstand, 5–50 mm).
- Lage pro Teil in den Eigenschaften: **Vorne** vor dem Avatar, **Hinten** dahinter (von hinten gesehen wie gezeichnet) oder **Oben (fallen lassen)** waagerecht über dem Avatar. Wo das Teil im Schnittfenster auf der Silhouette liegt, da liegt es auch in 3D.
- Stoffart pro Teil: Baumwolle, Seide, Jeans, Strick (dehnbar), Leder. Sie unterscheiden sich in Gewicht, Steifigkeit, Dehnbarkeit und Reibung.

**Simulation (Play)**
- Play in der 3D-Leiste, die Leertaste oder Simulation > Starten: Der Stoff fällt, legt sich um den Avatar, die Nähte ziehen sich zusammen. Noch einmal hält an. Der Stoff behält dann seine Lage.
- Zurücksetzen legt den Stoff wieder flach in die Anfangslage.
- Nähte ziehen sich mit begrenzter Geschwindigkeit zusammen, nicht auf einen Schlag. Vorder- und Rückenteil, die 40 cm auseinander anfangen, schließen sich so ohne Ruck.
- Kollision mit dem Avatar und dem Boden. Ausgeblendete Avatar-Teile, etwa Haare, zählen nicht. Der Stoff legt sich dann direkt auf den Kopf.
- Ändert man den Schnitt, wird der Stoff neu aufgebaut.

**Auswählen in 3D**
- Ein Klick auf den Stoff wählt das Schnittteil aus, auch im 2D-Fenster und in den Eigenschaften. Es wird gelb markiert. Wird ein Teil im Schnitt ausgewählt, leuchtet es auch in 3D.

**Noch nicht**
- Der Stoff stößt sich nicht an sich selbst. Lagen können sich durchdringen, etwa ein Rock über einer Hose aus demselben Projekt.
- Am Stoff mit der Maus ziehen, Anordnungspunkte um den Körper und Simulation auf der Grafikkarte kommen später.
- Getestet mit einem Tuch über dem Kopf der Testfigur und automatischen Tests (Tuch fällt heil auf den Boden, Naht schließt sich von 42 cm auf unter 2 cm, Tuch bleibt auf dem Kopf liegen). Noch nicht mit einem ganzen Kleidungsstück an einem echten Avatar.

## 0.1.7

**Avatar-Teile anklicken**
- Ein Klick auf den Avatar in der 3D-Ansicht markiert den getroffenen Teil hellgelb, etwa den Oberkörper, die Haare oder ein mitgeliefertes Kleidungsstück. Ein Klick daneben hebt die Markierung auf. Ziehen dreht weiterhin die Kamera.
- Die Eigenschaften zeigen dann Name, Dreiecke, Texturgröße und ob der Teil ausgestanzt ist.
- Teile lassen sich ausblenden, einzeln in den Eigenschaften oder über die Häkchen in der Objektliste. Gedacht für Haare oder mitgelieferte Kleidung, die beim Anpassen stören. Ausgeblendete Teile verschwinden auch aus der Silhouette im Schnittfenster und lassen sich nicht anklicken.
- In der Objektliste stehen unter dem Avatar alle Teile mit den Namen aus der Datei (Netz und Material). Teile ohne Namen werden durchnummeriert.

**Anzeige-Symbole über den Ansichten**
- Die Schalter für Avatar, Texturen, Boden, Schatten und Bildschirmfoto schweben jetzt wie im Original oben links über der 3D-Ansicht, die für Silhouette, Namen, Kantenlängen und Nähte über dem Schnittfenster. Die Leisten darüber enthalten nur noch die Werkzeuge.
- Die Mausposition im Schnittfenster steht jetzt unten links.

**Nicht geprüft**
- Mit echten Avataren aus vielen Teilen. Getestet mit der Testfigur aus zwei Teilen.

## 0.1.6

**Optik wie in Marvelous Designer**
- 3D-Fenster: grauer Verlaufshintergrund statt Schwarz, Bodenraster und ein Schatten des Avatars auf dem Boden. Avatare ohne Textur sind fast weiß wie eine Schneiderpuppe, vorher hellgrau.
- 2D-Schnittfenster: heller Hintergrund, die Avatar-Silhouette liegt über dem Raster, Schnittteile hell mit dunklem Umriss.
- Über dem 3D- und dem 2D-Fenster gibt es jetzt je zwei Symbolzeilen. Oben stehen die Werkzeuge bzw. Blickrichtungen, darunter die Anzeige-Schalter.
  - 3D: Ansicht von vorn, hinten, links, rechts, oben, Kamera auf Avatar. Darunter Avatar, Texturen, Boden, Schatten an/aus und Bildschirmfoto.
  - 2D: Silhouette, Namen der Teile, Kantenlängen an jeder Kante, Nähte an/aus.
- Menüleiste nach Themen wie im Original: Datei, Bearbeiten, 2D-Schnitt, Nähen, Avatar, Anzeige, Fenster, Hilfe. Menüs für Funktionen, die noch fehlen (3D-Kleidung, Materialien, Retopologie, Animation), kommen mit diesen Funktionen.
- Statusleiste: links Version und Projektname (mit Stern, wenn ungespeichert), rechts Knöpfe für die Aufteilung: 3D und 2D, nur 3D, nur 2D.
- Nähen > Alle Nähte löschen.

## 0.1.5

**Avatare ohne Textur waren schwarz** (seit 0.1.2)
- Alles ohne Textur erschien in 3D als schwarze Fläche: Avatare ohne Texturen und Teile, deren Textur nicht gefunden wurde. Die weiße Ersatztextur kam beim Zeichnen als Schwarz an. Teile ohne Textur verwenden ihre Materialfarbe jetzt direkt, ohne Ersatztextur.
- Aufgefallen ist das erst jetzt, weil die Bildschirmfoto-Tests seit 0.1.2 nur texturierte Figuren geprüft haben.

**Import-Fenster**
- Avatar laden, eine Datei ins Fenster ziehen oder „Neu importieren“ in den Eigenschaften öffnet ein Fenster mit den Einstellungen:
  - Einheit aus der Datei übernehmen, oder von Hand: Millimeter, Zentimeter, Meter, Zoll, Fuß. Dazu eine zusätzliche Skalierung in Prozent.
  - Achsen aus der Datei übernehmen, oder von Hand: welche Achse der Datei zu X, Y (oben) und Z wird, jeweils umkehrbar.
  - Am Boden ausrichten.
  - Zurücksetzen, OK, Abbrechen.
- Gedacht für Dateien, bei denen die automatische Erkennung danebenliegt, etwa ein OBJ in Millimetern oder ein Export mit falsch gesetzter Hochachse.
- Die Einstellungen werden im Projekt gespeichert und beim Öffnen wieder verwendet. Das nächste Import-Fenster schlägt die zuletzt gewählten vor.
- Bei FBX, DAE und USD greift „von Hand“ auf die Rohzahlen der Datei, ohne die automatische Umrechnung.

**Nicht enthalten**
- Optionen für Kamera, Animation, Fitting Suit, Anordnungspunkte und Kleidung-Importieren gibt es nicht, weil Stoffwerk diese Dinge noch nicht kann. Schalter ohne Wirkung wären irreführend.

## 0.1.4

**Symbolleiste im Schnittfenster**
- Statt Textknöpfen gibt es kleine Strich-Symbole in einer Zeile, wie man es von Marvelous Designer kennt. Name, Kürzel und Kurzhilfe erscheinen beim Darüberfahren.
- Die Symbole sind eigene Zeichnungen, nicht die Grafiken von Marvelous Designer, die CLO gehören. Sie werden als Vektoren gezeichnet und bleiben bei jeder Bildschirmskalierung scharf.

## 0.1.3

Abschnitt 2: Der Schnitt-Editor. Im 2D-Fenster kann man jetzt Schnittteile zeichnen, bearbeiten und Nähte festlegen.

**Werkzeuge** (Leiste über dem 2D-Fenster, Kürzel in Klammern)
- Auswählen und Verschieben (A), Punkte und Kanten bearbeiten (Z), Punkt auf einer Kante einfügen (X).
- Polygon (H): Mit Umschalt gesetzte Punkte werden Kurvenpunkte. Ein Klick auf den ersten Punkt oder Enter schließt das Teil. Rechteck (S) und Kreis/Ellipse (E) zieht man auf.
- Innenlinie (I) in ein Teil zeichnen.
- Naht (N): erst eine Kante anklicken, dann die Gegenkante, jeweils nahe dem Ende, das zusammengehört. Beide Kanten erscheinen in derselben Farbe, feine Striche zeigen, welche Stellen aufeinandertreffen. Weichen die Längen um mehr als 0,5 cm ab, steht das in den Eigenschaften, weil der Stoff dort später gerafft wird.

**Rechtsklick-Menü wie im Original**
- Auf einem Punkt: Punkt löschen, Abnäher einfügen, in Kurvenpunkt bzw. Eckpunkt umwandeln.
- Auf einem Teil: Duplizieren, symmetrisch kopieren (verknüpft), Verknüpfung lösen, spiegeln, 90° drehen, löschen.
- Abnäher-Fenster mit Gesamtbreite und Länge. Der Punkt wird zur Mitte des Abnähers, die Spitze zeigt ins Teil.

**Symmetrische Teile**
- Eine verknüpfte Kopie übernimmt jede Änderung gespiegelt: Punkte ziehen, Abnäher, Innenlinien, Punkte einfügen und löschen. So entstehen linke und rechte Hälften, die immer gleich bleiben.

**Ansicht**
- Die Silhouette des Avatars liegt in echter Größe im Hintergrund. Die Füße stehen auf der Nulllinie, 1 cm im Schnitt ist 1 cm am Körper.
- „Teilen“ zeigt zwei Ansichten übereinander (Übersicht und Ausschnitt), jede mit eigenem Zoom.
- Kantenlängen beim Darüberfahren, Länge der aktuellen Linie beim Zeichnen, Maße beim Aufziehen.
- Raster: Punkte rasten auf 0,5 cm ein, Strg hält das kurz aus.
- Eigenschaften rechts: Name, Breite, Höhe, Umfang und Fläche jedes Teils, die Längen aller Kanten, Punkte mit exakten Koordinaten.

**Rückgängig und Projekte**
- Rückgängig und Wiederholen (Strg+Z / Strg+Y) für jede Änderung, bis zu 200 Schritte.
- Projekt öffnen und speichern (.stoffwerk, Strg+O / Strg+S). Darin stehen der Schnitt und der Pfad zum Avatar, der beim Öffnen mitgeladen wird. Liegt der Avatar neben dem Projekt, wird der Pfad relativ gespeichert, damit man den Ordner verschieben kann.
- Beim Beenden, bei „Neues Projekt“ und beim Öffnen fragt Stoffwerk nach, wenn Ungespeichertes verloren ginge. Im Fenstertitel steht dann ein Stern.
- Das Format ist lesbares JSON.

**Nicht geprüft**
- Tastenkürzel: Im Test kamen keine Tasten an, weil das Testfenster keinen Fokus hatte. Die Werkzeugknöpfe sind geprüft, die Kürzel nicht.
- Kurvenpunkte und Innenlinien sind nur mit Unit-Tests geprüft, nicht in der Bedienung.
- Die Schnittteile erscheinen noch nicht in 3D. Das kommt mit der Simulation (Abschnitt 3).

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
