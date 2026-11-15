# Construction toolbar rows

Reference inspected: the owner's image `image(20261115-095411).png` and public
TrainsWorld `trainworld/app/src/ui/ToolBar.tsx` at
https://github.com/circularsquare/trainsworld/blob/main/trainworld/app/src/ui/ToolBar.tsx
(fetched 2026-11-15). The reference uses explicit rows and expands advanced speed
controls beneath its main controls. No source code was copied.

The owner's later request specifically moves both track count and 詳細設定 down
one row. Rosenfu therefore uses: height row; type/following row; count/details row.
The native details disclosure starts closed, expands downward and retains values
when closed again. No height control or cost feature is changed.

Rows wrap within the map width with separate explicit containers rather than
relying on accidental flex overflow. DOM order, common lower-row membership,
responsive CSS rules and disclosure/value interactions are tested. Native browser
pixel layout and keyboard activation remain unverified in this environment.
