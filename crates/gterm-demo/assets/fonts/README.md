# Fonts

The files in this directory are third-party works. They are **not** covered by the license
of `gterm`; each keeps its own license.

## RobotoMono Nerd Font Mono

Files: `RobotoMonoNerdFontMono-{Regular,Bold,Italic,BoldItalic}.ttf`
(version `3.000;Nerd Fonts 3.5.1`).

- Base font: Roboto Mono — Copyright 2015 The Roboto Mono Project Authors
  (<https://github.com/googlefonts/robotomono>), licensed under the Apache License, Version 2.0.
  The license text is in [`LICENSE-APACHE-2.0.txt`](LICENSE-APACHE-2.0.txt).
  Roboto Mono is a trademark of Google.
- The font was patched with additional glyphs by the Nerd Fonts project
  (<https://github.com/ryanoasis/nerd-fonts>). The added glyph sets come from several upstream
  projects, each under its own license; the list is maintained by Nerd Fonts in
  <https://github.com/ryanoasis/nerd-fonts/blob/master/license-audit.md>.

The font files are used unmodified. They are needed only by the demo binaries and by the tests of
the `gterm` crate; the library itself embeds no font.
