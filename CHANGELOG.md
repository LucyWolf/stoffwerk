# Changelog

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
