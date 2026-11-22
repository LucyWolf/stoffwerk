# Testdateien

Kleine Figuren (1,75 m) in jedem Format, das Stoffwerk lädt. Der Release-Workflow lädt sie mit
`stoffwerk --pruefen tests/dateien/*.*`. Jede muss 1,75 m hoch ankommen.

| Datei | prüft |
|---|---|
| figur_cm.obj | Zentimeter werden zu Metern |
| figur_tex.obj/.mtl | Texturpfad von einem fremden Windows-Rechner wird über den Dateinamen gefunden |
| figur_zup_cm.fbx | FBX mit Z oben und Zentimetern |
| figur.dae | Collada mit Z oben, Zentimetern, Textur über sampler → surface → image |
| figur.usda | USD mit Z oben, metersPerUnit 0.01, UsdPreviewSurface + UsdUVTexture |
| figur.glb | eingebettete Texturen, Haar-Teil mit alphaMode MASK (ausgestanzt) |
| figur_tpose_katze.obj | Simulation: T-Pose, Kugelkopf mit falsch herum zeigenden Flächen, einseitige Ohren und Schnurrhaare (Tuch darf nicht durchfallen oder knüllen) |
