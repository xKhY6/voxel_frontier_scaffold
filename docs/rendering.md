# Notes de rendu

## État actuel

- `src/render/` : contexte OpenGL (`context.rs`) et `Renderer` qui dessine un maillage GPU par chunk (`mod.rs`).
- `assets/shaders/` contient `cube.vert` et `cube.frag` ; `assets/textures/` est vide.
- Dépendances graphiques : glow, glutin, glutin-winit, raw-window-handle (voir [DEPENDENCIES.md](../DEPENDENCIES.md)).

## Choix retenus

- Backend : OpenGL 3.3 core. Le contexte est créé par glutin (lié à winit par glutin-winit) ; les appels passent par glow et sont `unsafe`, confinés à `render/`.
- Shaders GLSL 330 core dans `assets/shaders/`, embarqués avec `include_str!`.
- Profondeur en -1..1 : la projection de `camera/` utilise `Mat4::perspective_rh_gl`.
- Maillage (`voxel/mesh.rs`) : seules les faces dont le voisin est de l'air ou dans un chunk non chargé sont émises ; 4 sommets et 6 index par face. Couleur par type de bloc, ombrage par direction (dessus 1.0, côtés X 0.8, côtés Z 0.65, dessous 0.5).
- Culling des faces arrière activé : les triangles sont antihoraires vus de l'extérieur (test d'orientation dans `voxel/mesh.rs`).
- Un maillage GPU par chunk, stocké dans une table indexée par coordonnée ; un maillage vide retire le chunk de l'affichage.

## À décider

- Rechargement des shaders à chaud : hors MVP.
- Textures : à traiter avec les blocs texturés.
