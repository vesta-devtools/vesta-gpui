# Shared GPUI test font

`MaterialIcons-Regular.ttf` is a byte-identical test fixture from Google
`material-design-icons` revision `50f0603134ce7b70b2d71b686cc13e8b57ccb74c`:

<https://github.com/google/material-design-icons/blob/50f0603134ce7b70b2d71b686cc13e8b57ccb74c/font/MaterialIcons-Regular.ttf>

- SHA-256: `EF149F08BDD2FF09A4E2C8573476B7B0F3FBB15B623954ADE59899E7175BEDDA`.
- License: Apache License 2.0; see `LICENSE-Material-Icons.txt`.
- Scope: shared library tests only. Production hosts continue to supply their own fonts.

Keeping the fixture in the SDK removes the shared tests' dependency on the Studio
application directory. It does not enable system font discovery or change text
layout, glyphs, shader behavior or the production font policy.
